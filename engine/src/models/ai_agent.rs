use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAgentProtocol {
    OpenaiResponses,
    OpenaiChatCompletions,
    AnthropicMessages,
    GeminiGenerateContent,
    Custom,
}

impl<'de> Deserialize<'de> for AiAgentProtocol {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "openai_responses" => Ok(Self::OpenaiResponses),
            "openai_chat_completions" => Ok(Self::OpenaiChatCompletions),
            "anthropic_messages" => Ok(Self::AnthropicMessages),
            "gemini_generate_content" => Ok(Self::GeminiGenerateContent),
            "custom" => Ok(Self::Custom),
            _ => Err(de::Error::custom("unsupported AI Agent protocol")),
        }
    }
}

// Struct-derived serde otherwise also accepts positional arrays. Metadata contracts are
// objects only and retain serde's duplicate/unknown-field rejection at each object level.
macro_rules! metadata_object {
    ($(#[$attribute:meta])* $name:ident { $( $field:ident: $ty:ty ),* $(,)? }) => {
        $(#[$attribute])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct $name { $(pub $field: $ty),* }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Fields { $(#[serde(deserialize_with = "required_field")] $field: $ty),* }
                struct Object;
                impl<'de> Visitor<'de> for Object {
                    type Value = $name;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("AI Agent metadata object")
                    }
                    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                        let fields = Fields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                        Ok($name { $($field: fields.$field),* })
                    }
                }
                deserializer.deserialize_map(Object)
            }
        }
    };
}

fn required_field<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    T::deserialize(deserializer)
}

metadata_object!(AiAgentProfile {
    id: String, name: String, protocol: AiAgentProtocol, base_url: String,
    model: String, credential_ref: Option<String>, enabled: bool,
});
metadata_object!(#[derive(Default)] AiAgentSettings {
    revision: u32, default_profile_id: Option<String>, profiles: Vec<AiAgentProfile>,
});
metadata_object!(UpdateAiAgentSettingsRequest {
    expected_revision: u32, default_profile_id: Option<String>, profiles: Vec<AiAgentProfile>,
});

#[derive(Debug, Serialize)]
pub struct UpdateAiAgentSettingsResponse {
    pub ok: bool,
    pub settings: AiAgentSettings,
}
