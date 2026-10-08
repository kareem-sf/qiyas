//! The in-app assistant's state: one `Agent` per conversation, built at its first message from the
//! settings and the keychain, and lent to the background task that runs each turn.
//!
//! A new conversation (Clear, another drawing, another language, other AI settings or key) drops
//! the agent, so the next message builds a fresh one with a new system prompt and the current key.

use std::sync::{Arc, Mutex};

use qiyas_ai::anthropic::Anthropic;
use qiyas_ai::openai::OpenAiCompatible;
use qiyas_ai::prompt::Lang;
use qiyas_ai::{Agent, AgentSettings, Provider};
use qiyas_core::Session;
use tokio_util::sync::CancellationToken;

use crate::settings::{self, Settings};

#[derive(Default)]
pub struct AiRun {
    /// The conversation; `None` before its first message, and while a turn runs (the task holds
    /// it).
    agent: Option<Agent>,
    /// Cancels the running turn.
    cancel: Option<CancellationToken>,
    busy: bool,
    /// Bumped by every new conversation. A turn started in an older one drops its agent and its
    /// events.
    generation: u64,
    /// The language of the system prompt, set by the window.
    language: Lang,
    /// Messages in the conversation after the last finished turn.
    messages: usize,
}

/// A turn handed to the background task.
pub struct Turn {
    pub agent: Agent,
    pub cancel: CancellationToken,
    pub generation: u64,
}

impl AiRun {
    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn message_count(&self) -> usize {
        self.messages
    }

    pub fn language(&self) -> Lang {
        self.language
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Cancels a running turn and forgets the conversation.
    pub fn new_conversation(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        self.agent = None;
        self.generation += 1;
        self.messages = 0;
    }

    /// Sets the prompt language; a change starts a new conversation. True when it changed.
    pub fn set_language(&mut self, lang: Lang) -> bool {
        if lang == self.language {
            return false;
        }
        self.language = lang;
        self.new_conversation();
        true
    }

    /// Claims the next turn: `Err("busy")` while one runs. Gives the conversation's agent (`None`
    /// when one must be built) and the generation to pass to `start`.
    pub fn reserve(&mut self) -> Result<(Option<Agent>, u64), String> {
        if self.busy {
            return Err("busy".into());
        }
        self.busy = true;
        Ok((self.agent.take(), self.generation))
    }

    /// Gives up a claimed turn that could not start (no key, say).
    pub fn release(&mut self, agent: Option<Agent>, generation: u64) {
        self.busy = false;
        if generation == self.generation {
            self.agent = agent;
        }
    }

    /// Starts the claimed turn with `agent`, unless a new conversation began since `reserve`.
    pub fn start(&mut self, agent: Agent, generation: u64) -> Result<Turn, String> {
        if generation != self.generation {
            self.busy = false;
            return Err("A new conversation started; send the message again".into());
        }
        let cancel = CancellationToken::new();
        self.cancel = Some(cancel.clone());
        Ok(Turn {
            agent,
            cancel,
            generation,
        })
    }

    /// Takes the agent back after a turn; it is dropped when a new conversation began meanwhile.
    pub fn finish(&mut self, agent: Agent, generation: u64) {
        self.busy = false;
        self.cancel = None;
        if generation == self.generation {
            self.messages = agent.history().len();
            self.agent = Some(agent);
        }
    }

    /// Cancels the running turn, if any.
    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
    }
}

/// What the agent needs from the settings, in the window's language.
pub fn agent_settings(s: &Settings, language: Lang) -> AgentSettings {
    AgentSettings {
        model: s.model.clone(),
        effort: s.effort.clone(),
        language,
        ..AgentSettings::default()
    }
}

/// Whether going from `old` to `new` changes who answers (provider, address or model), which needs
/// a new conversation. Effort changes apply to the next message.
pub fn answerer_changed(old: &Settings, new: &Settings) -> bool {
    (old.provider, &old.base_url, &old.model) != (new.provider, &new.base_url, &new.model)
}

/// The provider the settings name, with `key` from the keychain. `Err("no_api_key")` for Claude
/// without a key and `Err("no_base_url")` for an OpenAI-compatible service without an address;
/// the window opens the settings for both.
pub fn build_provider(s: &Settings, key: Option<String>) -> Result<Box<dyn Provider>, String> {
    match s.provider {
        settings::Provider::Anthropic => {
            let key = key.ok_or("no_api_key")?;
            Ok(Box::new(Anthropic::new(key)))
        }
        settings::Provider::OpenaiCompatible => {
            let url = s
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .ok_or("no_base_url")?;
            Ok(Box::new(OpenAiCompatible::new(url, key)))
        }
    }
}

/// The canned provider of `QIYAS_AI_MOCK=1`, in debug builds only.
pub fn mock_provider(session: &Arc<Mutex<Option<Session>>>) -> Option<Box<dyn Provider>> {
    #[cfg(debug_assertions)]
    if std::env::var("QIYAS_AI_MOCK").as_deref() == Ok("1") {
        return Some(Box::new(crate::mock::Mock::new(session.clone())));
    }
    let _ = session;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> Agent {
        let provider = build_provider(&Settings::default(), Some("k".into())).unwrap();
        Agent::new(
            provider,
            Arc::new(Mutex::new(None)),
            AgentSettings::default(),
        )
    }

    #[test]
    fn one_turn_at_a_time_and_the_agent_comes_back() {
        let mut run = AiRun::default();
        let (none, generation) = run.reserve().unwrap();
        assert!(none.is_none(), "the first message builds the agent");
        assert_eq!(run.reserve().err().as_deref(), Some("busy"));
        let turn = run.start(agent(), generation).unwrap();
        assert!(run.busy());
        run.finish(turn.agent, turn.generation);
        assert!(!run.busy());
        let (kept, _) = run.reserve().unwrap();
        assert!(kept.is_some(), "the conversation is kept between turns");
    }

    #[test]
    fn a_new_conversation_cancels_the_turn_and_drops_its_agent() {
        let mut run = AiRun::default();
        let (_, generation) = run.reserve().unwrap();
        let turn = run.start(agent(), generation).unwrap();
        run.new_conversation();
        assert!(turn.cancel.is_cancelled());
        run.finish(turn.agent, turn.generation);
        assert!(!run.busy());
        let (agent, _) = run.reserve().unwrap();
        assert!(agent.is_none(), "the next message starts afresh");
    }

    #[test]
    fn a_reset_between_reserve_and_start_refuses_the_turn() {
        let mut run = AiRun::default();
        let (_, generation) = run.reserve().unwrap();
        run.new_conversation();
        assert!(run.start(agent(), generation).is_err());
        assert!(!run.busy());
    }

    #[test]
    fn only_a_new_language_starts_a_new_conversation() {
        let mut run = AiRun::default();
        assert!(!run.set_language(Lang::En));
        let before = run.generation();
        assert!(run.set_language(Lang::Ar));
        assert_eq!(run.generation(), before + 1);
        assert_eq!(run.language(), Lang::Ar);
    }

    #[test]
    fn providers_need_a_key_or_an_address() {
        let mut s = Settings::default();
        assert_eq!(
            build_provider(&s, None).err().as_deref(),
            Some("no_api_key")
        );
        s.provider = settings::Provider::OpenaiCompatible;
        assert_eq!(
            build_provider(&s, None).err().as_deref(),
            Some("no_base_url")
        );
        s.base_url = Some("http://localhost:11434/v1".into());
        assert!(
            build_provider(&s, None).is_ok(),
            "a local server may take no key"
        );
    }

    #[test]
    fn effort_keeps_the_conversation_and_model_does_not() {
        let old = Settings::default();
        let mut new = old.clone();
        new.effort = "low".into();
        assert!(!answerer_changed(&old, &new));
        new.model = "claude-sonnet-5-5".into();
        assert!(answerer_changed(&old, &new));
    }
}
