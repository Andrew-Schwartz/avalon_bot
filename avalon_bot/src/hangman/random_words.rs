use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::Client;
use serde::Deserialize;

use discorsd::model::channel::TextChannel;
use discorsd::model::guild::Guild;
use discorsd::model::ids::{GuildId, Id, MessageId};

#[derive(Debug)]
pub struct GuildHist {
    guild: GuildId,
    channels: Vec<TextChannel>,
    idx: usize,
}

impl Id for GuildHist {
    type Id = GuildId;

    fn id(&self) -> Self::Id {
        self.guild
    }
}

impl PartialEq for GuildHist {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl GuildHist {
    // todo remove this when impl'd
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(_guild: Guild) -> Self {
        // let channels = guild.channels
        //     .into_iter()
        //     .filter(|c| c.channel_type() == ChannelType::GuildText)
        //     .collect();
        // Self {
        //     guild: guild.id,
        //     channels,
        //     idx: 0,
        // }
        todo!()
    }

    pub async fn word(&mut self) -> (String, String) {
        loop {
            let channel = &self.channels[self.idx];
            let channel_creation = channel.id.timestamp().timestamp();
            let now = Utc::now().timestamp();

            let mut rng = rand::thread_rng();
            let rand_time = rng.gen_range(channel_creation..now);
            let time = NaiveDateTime::from_timestamp(rand_time, 0);
            let message_id_around: MessageId = time.into();

            println!("message_id_around = {:?}", message_id_around);
        }

        // todo!()
    }
}

static URL: Lazy<String> = Lazy::new(|| {
    let key = std::fs::read_to_string("wordnik.txt").unwrap();
    format!("https://api.wordnik.com/v4/words.json/randomWord?api_key={}", key)
});

#[derive(Debug, Default)]
pub struct Wordnik(Client);

impl Wordnik {
    pub async fn word(&self) -> (String, String) {
        #[derive(Deserialize)]
        struct Word { word: String }

        async fn word_(client: &Client) -> Option<String> {
            let word: Word = client.get(&*URL)
                .send().await.ok()?
                .json().await.ok()?;
            Some(word.word)
        }
        loop {
            if let Some(word) = word_(&self.0).await {
                if word.chars().all(|c| matches!(c, 'a'..='z')) {
                    let source = format!("https://www.wordnik.com/words/{}", word);
                    break (word, source);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}