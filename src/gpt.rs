use crate::config::GptConfig;
use anyhow::{bail, Context, Result};
use serde::*;

const PROMPT: &str = r#"
# 任务
从一个动漫下载网页的标题中提取出其中的信息。只输出你提取的信息，不要输出其他的格式。

## 输入
多行文本，每行都是一个动漫下载页面的标题。

## 输出
对输入的每一行，从标题中提取出：字幕组、动漫的名字、季数、集数、分辨率、字幕语言。
将所有的结果按照顺序放在一个 json array 中输出，不要用 ``` 包起来。

其中，
- 动漫的名字使用标题中的名字。请优先使用翻译的中文名字，如果没有就使用原标题中出现的原名。要确保名字中不要存在 /、\ 等文件名中不安全的字符。你可以使用形状相似的字符进行替代。
- 分辨率使用 720p 或者 1080p。
- 字幕语言使用“生肉、简体中文、繁体中文、简繁中文、简日双语、繁日双语”。生肉指没有翻译、字幕。
- 集数如果是合集，则输出如 1-24。

## 例子
一个可能的输入： [LoliHouse] 治愈魔法的错误使用方法 / Chiyu Mahou no Machigatta Tsukaikata - 09 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕] 
输出：
```
[{
    "fansub": "LoliHouse",
    "anime": "治愈魔法的错误使用方法",
    "season": "1",
    "episode": "9",
    "resolution": "1080p",
    "language": "简体中文"
}]
```"#;
const PER_REQ_SIZE: usize = 5;

#[derive(Debug, Deserialize)]
pub struct RecognizedResult {
    /// 字幕组
    pub fansub: String,
    pub anime: String,
    pub season: String,
    pub episode: String,
    pub resolution: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
struct Response {
    usage: Usage,
    choices: Vec<Choice>,
}
#[derive(Debug, Deserialize)]
#[allow(unused)]
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}
#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

pub async fn get_episode_info(
    titles: &[String],
    config: &GptConfig,
) -> Result<Vec<RecognizedResult>> {
    let mut futures = vec![];
    for chunk in titles.chunks(PER_REQ_SIZE) {
        futures.push(get_episode_info_with_retry(chunk, config));
    }
    let re = futures::future::try_join_all(futures).await?;
    let re = re.into_iter().flatten().collect();
    Ok(re)
}

async fn get_episode_info_with_retry(
    titles: &[String],
    config: &GptConfig,
) -> Result<Vec<RecognizedResult>> {
    for i in 0..=config.retry {
        let model = config.model(i);
        debug!("get_episode_info_with_retry, i={i}, model={model}");
        let r = get_episode_info_raw(titles, model, config).await;
        match r {
            Ok(r) => return Ok(r),
            Err(e) => {
                warn!("err={e:#}");
                if i == config.retry {
                    debug!("get_episode_info_with_retry, max retry reached, i={i}");
                    return Err(e);
                } else {
                    debug!("get_episode_info_with_retry, retry. i={i}");
                    continue;
                }
            }
        }
    }
    unreachable!()
}

async fn get_episode_info_raw(
    titles: &[String],
    model: &str,
    config: &GptConfig,
) -> Result<Vec<RecognizedResult>> {
    let client = reqwest::Client::new();
    debug!(
        "asking gpt {model} to recognize {} items: {titles:?}",
        titles.len()
    );

    let t = std::time::Instant::now();
    let response = client
        .post(&config.url)
        .bearer_auth(&config.token)
        .json(&serde_json::json!({
            "model": &model,
            "temperature": 0.2,
            "messages": [
                {"role": "system", "content": PROMPT },
                {"role": "user", "content": titles.join("\n") }
            ]
        }))
        .send()
        .await?;
    debug!(
        "gpt response status: {}, time cost: {:?}",
        response.status(),
        t.elapsed()
    );
    if !response.status().is_success() {
        let body = response.text().await?;
        error!("gpt response: {body}");
        bail!("gpt failed: {body}")
    }

    let mut response = response
        .json::<Response>()
        .await
        .context("parse open ai response failed")?;
    info!("response got, response={:?}", response.usage);

    let content = response.choices.pop().context("no msg")?.message.content;
    trace!("content: {}", content);
    let content = content.trim().trim_matches('`');
    let content =
        serde_json::from_str::<Vec<RecognizedResult>>(content).context("parse as result failed")?;
    info!(
        "ok, time cost: {:?}, titles = {:?}, gpt result: {:?}",
        t.elapsed(),
        titles,
        content
    );
    if content.len() != titles.len() {
        debug!("titles (len={}) = {titles:?}", titles.len());
        debug!("content (len={}) = {content:?}", content.len());
        bail!("length not match, input={}, output={}", titles.len(), content.len());
    }

    Ok(content)
}
