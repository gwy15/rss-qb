use crate::config::GptConfig;
use anyhow::{bail, Context, Result};
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{config::OpenAIConfig, Client};
use serde::*;

type GptClient = async_openai::Client<async_openai::config::OpenAIConfig>;

const PROMPT: &str = include_str!("prompt.txt");
const PER_REQ_SIZE: usize = 6;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Recognized {
    Show(ShowInfo),
    Other,
}

#[derive(Debug, Deserialize)]
pub struct ShowInfo {
    pub fansub: String,
    pub show: String,
    pub season: i64,
    pub episode: i64,
    pub resolution: String,
    pub language: String,
    #[serde(skip_deserializing)]
    pub year: i64,
    #[serde(skip_deserializing)]
    pub tmdb_id: i64,
}

pub async fn get_episode_info(titles: &[String], config: &GptConfig) -> Result<Vec<Recognized>> {
    let client_config = OpenAIConfig::new()
        .with_api_key(&config.token)
        .with_api_base(&config.url);
    let client = Client::with_config(client_config);

    let mut futures = vec![];
    for chunk in titles.chunks(PER_REQ_SIZE) {
        futures.push(get_episode_info_with_retry(chunk, &client, config));
    }
    let re = futures::future::try_join_all(futures).await?;
    let re = re.into_iter().flatten().collect();
    Ok(re)
}

async fn get_episode_info_with_retry(
    titles: &[String],
    client: &GptClient,
    config: &GptConfig,
) -> Result<Vec<Recognized>> {
    for i in 0..=config.retry {
        let model = config.model(i);
        debug!("get_episode_info_with_retry, i={i}, model={model}");
        let r = get_episode_info_raw(titles, client, model).await;
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
    client: &GptClient,
    model: &str,
) -> Result<Vec<Recognized>> {
    debug!(
        "asking gpt {model} to recognize {} items: {titles:?}",
        titles.len()
    );
    let t = std::time::Instant::now();

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        // .response_format(types::ChatCompletionResponseFormat { // 3.5-turbo-0125 用这个会有问题
        //     r#type: types::ChatCompletionResponseFormatType::JsonObject,
        // })
        .temperature(0.2)
        .messages(vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(PROMPT)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(titles.join("\n"))
                .build()?
                .into(),
        ])
        .build()?;
    let mut response = client.chat().create(request).await?;
    info!("gpt response got, usage = {:?}", response.usage);

    let choice = response.choices.pop().context("no choice?")?;
    let content = choice.message.content.unwrap_or_default();
    let content = content
        .trim()
        .trim_matches('`')
        .trim_start_matches("json")
        .trim();

    let content = serde_json::from_str::<Vec<Recognized>>(content)
        .with_context(|| format!("parse as result failed: content = {content:?}"))?;

    if content.len() != titles.len() {
        debug!("titles (len={}) = {titles:?}", titles.len());
        debug!("content (len={}) = {content:?}", content.len());
        bail!(
            "length not match, input={}, output={}",
            titles.len(),
            content.len()
        );
    }
    info!(
        "ok, time cost: {:?}, recognized {} items.",
        t.elapsed(),
        content.len()
    );

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse() {
        let s = r#"[
            { "type": "show", "fansub": "ANi", "show": "秒殺外掛太強了，異世界的傢伙們根本就不是對手。", "season": 1, "episode": 8, "resolution": "1080p", "language": "简繁中文" },
            { "type": "show", "fansub": "LoliHouse", "show": "秒杀外挂太强了，异世界的家伙们根本就不是对手。", "season": 1, "episode": 7, "resolution": "1080p", "language": "简繁中文" },
            { "type": "show", "fansub": "ANi", "show": "秒殺外掛太強了，異世界的傢伙們根本就不是對手。", "season": 1, "episode": 7, "resolution": "1080p", "language": "简繁中文" },
            { "type": "show", "fansub": "LoliHouse", "show": "秒杀外挂太强了，异世界的家伙们根本就不是对手。", "season": 1, "episode": 6, "resolution": "1080p", "language": "简繁中文" },
            { "type": "show", "fansub": "ANi", "show": "秒殺外掛太強了，異世界的傢伙們根本就不是對手。", "season": 1, "episode": 6, "resolution": "1080p", "language": "简繁中文" }
        ]"#;
        let _ = serde_json::from_str::<Vec<Recognized>>(s).unwrap();
    }
}
