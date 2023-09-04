use chrono::{NaiveDateTime, Utc};
use discorsd::BotState;
use discorsd::errors::BotError;
use discorsd::http::channel::GetMessages;
use discorsd::model::ids::{ChannelId, GuildId, Id, MessageId};
use itertools::Itertools;
use once_cell::sync::Lazy;
use rand::{Rng, thread_rng};
use rand::prelude::SliceRandom;
use reqwest::Client;
use discorsd::model::channel::ChannelType;
use serde::Deserialize;
use serde_derive::Deserialize;

use crate::Bot;
use crate::error::{GameError, HangmanError};

const MIN_WORD_LEN: usize = 5;

pub async fn channel_hist_word(state: &BotState<Bot>, channel: ChannelId, guild: Option<GuildId>) -> Result<(String, String), BotError<GameError>> {
    let channel_creation = channel.timestamp().timestamp();
    println!("channel = {:?}", channel);
    let now = Utc::now().timestamp();

    let rand_time = {
        let mut rng = thread_rng();
        rng.gen_range(channel_creation..now)
    };
    println!("rand_time = {:?}", rand_time);
    let time = NaiveDateTime::from_timestamp_opt(rand_time, 0).unwrap();
    println!("time = {:?}", time);
    let message = MessageId::from(time);
    println!("message = {:?}", message);

    let get = GetMessages::new().limit(100).around(message);
    let messages = state.client.get_messages(channel, get).await?;
    let mut rng = thread_rng();
    messages.into_iter()
        .find_map(|m| {
            let mut vec = m.content.split_ascii_whitespace()
                .filter(|s| s.chars().all(|c| c.is_ascii_alphabetic()))
                .filter(|s| s.len() >= MIN_WORD_LEN)
                .collect_vec();
            println!("vec = {:?}", vec);
            vec.shuffle(&mut rng);
            (!vec.is_empty()).then(|| (
                vec.swap_remove(0).to_ascii_lowercase(),
                match guild {
                    Some(guild) => format!("https://discord.com/channels/{guild}/{channel}/{}", m.id),
                    None => format!("https://discord.com/channels/@me/{channel}/{}", m.id)
                }
            ))
        })
        .ok_or_else(|| HangmanError::NoWords(channel, guild).into())
}

pub async fn server_hist_word(state: &BotState<Bot>, guild: Result<GuildId, ChannelId>) -> Result<(String, String), BotError<GameError>> {
    let (channel, guild) = match guild {
        Ok(guild) => {
            let guild = state.cache.guild(guild).await.unwrap();
            let mut channels = guild.channels.iter()
                .filter(|c| matches!(c.variant_type(), ChannelType::Text | ChannelType::Dm))
                .collect_vec();
            channels.shuffle(&mut thread_rng());
            (channels[0].id(), Some(guild.id))
        }
        Err(channel) => (channel, None),
    };
    channel_hist_word(state, channel, guild).await
}

static WORDNIK_URL: Lazy<String> = Lazy::new(|| {
    let key = std::fs::read_to_string("wordnik.txt").unwrap();
    format!(
        "https://api.wordnik.com/v4/words.json/randomWords?\
         hasDictionaryDef=true&\
         includePartOfSpeech=noun,adjective,verb,adverb,preposition&\
         minLength={MIN_WORD_LEN}&\
         limit=100&\
         api_key={key}"
    )
});

pub async fn wordnik_word(client: &Client) -> Result<(String, String), BotError<GameError>> {
    #[derive(Deserialize, Debug)]
    struct Word {
        word: String,
    }

    let words: Vec<Word> = client.get(&*WORDNIK_URL)
        .send().await?
        .json().await?;

    let word = words.into_iter()
        // all words from wordnik are lowercase
        .find(|w| w.word.chars().all(|c| c.is_ascii_alphabetic()))
        .unwrap()
        .word;

    let source = format!("https://www.wordnik.com/words/{word}");
    Ok((word, source))
}
