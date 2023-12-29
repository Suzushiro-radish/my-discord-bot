use anyhow::anyhow;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ChatCompletionRequestSystemMessageArgs,
    },
    Client as OpenAIClient,
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use shuttle_secrets::SecretStore;
use tracing::{error, info};

struct Bot {
    openai: OpenAIClient<OpenAIConfig>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if !msg.author.bot {
            let msg_history = msg
                .channel_id
                .messages(&ctx.http, |retriever| retriever.limit(8))
                .await
                .unwrap();
            let mut messages: Vec<ChatCompletionRequestMessage> = msg_history
                .iter()
                .map(|message| {
                    if message.author.bot {
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(message.content.as_str())
                            .build()
                            .unwrap()
                            .into()
                    } else {
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(message.content.as_str())
                            .build()
                            .unwrap()
                            .into()
                    }
                })
                .collect();

            let system_prompt: ChatCompletionRequestMessage = ChatCompletionRequestSystemMessageArgs::default()
            .content("あなたはマメゾンボヌールの使用人です。「でやんす」や「やんす」が語尾に付くような話し方をしてください。")
            .build()
            .unwrap()
            .into();

            messages.insert(0, system_prompt);

            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages(messages)
                .build()
                .unwrap();

            let response = self.openai.chat().create(request).await.unwrap();

            for choise in response.choices {
                if let Err(e) = msg
                    .channel_id
                    .say(&ctx.http, choise.message.content.unwrap())
                    .await
                {
                    error!("Error sending message: {:?}", e);
                }
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    let openai_api_key = if let Some(openai_api_key) = secret_store.get("OPENAI_KEY") {
        openai_api_key
    } else {
        return Err(anyhow!("'OPENAI_KEY' was not found").into());
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;

    let config = OpenAIConfig::default().with_api_key(openai_api_key);
    let openai_client = OpenAIClient::with_config(config);

    let client = Client::builder(&token, intents)
        .event_handler(Bot {
            openai: openai_client,
        })
        .await
        .expect("Err creating client");

    Ok(client.into())
}
