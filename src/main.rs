use justact_prototype::{
    auditing::{Event, EventControl},
    spec::collections::{map::InfallibleMap, Recipient},
    wire::{Action, Agreement, Message},
};
use std::{collections::HashSet, sync::Arc};

type Time = u64;
type StmtIdx = usize;
type AgreeIdx = usize;

struct Config {
    current: Time,
    statements: Vec<Arc<Message>>,
    agreements: Vec<Agreement>,
    enacted: Vec<Action>,
}

enum Cmd<'a> {
    Say { sayer: &'a str, payload: &'a str },
    Agree { on_idx: StmtIdx, at: Time },
    Enact { actor: &'a str, basis: AgreeIdx, justification: HashSet<StmtIdx> },
    Now { now: Time },
    Inspect,
}

impl<'a> Cmd<'a> {
    fn parse(input: &'a str) -> Option<Self> {
        let mut splits = input.splitn(3, char::is_whitespace);
        let cmd = splits.next()?;
        match cmd {
            "say" => {
                let sayer = splits.next()?;
                let payload = splits.next()?;
                Some(Self::Say { sayer, payload })
            }
            "agree" => {
                let on_idx: StmtIdx = splits.next()?.parse().ok()?;
                let at: Time = splits.next()?.parse().ok()?;
                Some(Cmd::Agree { on_idx, at })
            }
            "enact" => {
                let actor = splits.next()?;
                let rest = splits.next()?;
                let mut splits = rest.split(char::is_whitespace);
                let basis: AgreeIdx = splits.next()?.parse().ok()?;
                let justification: HashSet<StmtIdx> =
                    splits.map(|part| part.parse().ok()).collect::<Option<_>>()?;
                Some(Cmd::Enact { actor, basis, justification })
            }
            "now" => {
                let now: Time = splits.next()?.parse().ok()?;
                if splits.all(str::is_empty) {
                    Some(Cmd::Now { now })
                } else {
                    None
                }
            }
            "inspect" => Some(Cmd::Inspect),
            _ => None,
        }
    }
}

impl Config {
    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Say { sayer, payload } => self.statements.push(Arc::new(Message {
                id: (sayer.to_string(), self.statements.len().try_into().unwrap()),
                payload: payload.to_string(),
            })),
            Cmd::Agree { on_idx, at } => {
                if let Some(s) = self.statements.get_mut(on_idx) {
                    self.agreements.push(Agreement { at, message: s.clone() });
                } else {
                    println!("Limitation: cannot agree on unsaid messages!");
                }
            }
            Cmd::Enact { actor, basis, justification } => {
                if basis >= self.agreements.len() {
                    println!("Cannot be based using unsaid message {}", basis)
                } else if let Some(id) =
                    justification.iter().find(|&&id| id >= self.statements.len())
                {
                    println!("Cannot justify using unsaid message {}", id)
                } else {
                    self.enacted.push(Action {
                        id: (
                            actor.to_string(),
                            char::from_u32('a' as u32 + self.enacted.len() as u32)
                                .expect("out of bounds"),
                        ),
                        basis: self.agreements[basis].clone(),
                        justification: justification
                            .iter()
                            .map(|&idx| self.statements[idx].clone())
                            .collect(),
                    })
                }
            }
            Cmd::Now { now } => {
                self.current = now;
            }
            Cmd::Inspect => self.serialise(),
        }
    }

    fn serialise(&self) {
        let iter = std::iter::once(EventControl::AdvanceTime { timestamp: self.current })
            .chain(self.statements.iter().map(|s| EventControl::StateMessage {
                who: s.id.clone().0.into(),
                to: Recipient::All,
                msg: s.clone(),
            }))
            .chain(self.agreements.iter().map(|a| EventControl::AddAgreement { agree: a.clone() }))
            .chain(self.enacted.iter().map(|e| EventControl::EnactAction {
                who: e.id.0.clone().into(),
                to: Recipient::All,
                action: e.clone(),
            }));
        for c in iter {
            println!("{}", serde_json::to_string(&Event::Control(c)).expect("WAH"));
        }
    }
}

fn main() {
    let mut config = Config { current: 0, statements: vec![], agreements: vec![], enacted: vec![] };
    let mut buffer = String::new();
    loop {
        println!("current time: {}", config.current);
        if !config.statements.is_empty() {
            println!("___s_id__|___sayer___|___payload___");
            for (i, s) in config.statements.iter().enumerate() {
                println!("{: >8} | {: <9} | {:?}", i, s.id.0, s.payload);
            }
        }
        if !config.agreements.is_empty() {
            println!("___a_id___|___s_id___|___time___");
            for (i, a) in config.agreements.iter().enumerate() {
                println!("{: >8} | {: <9} | {:?}", i, a.message.id.1, a.at);
            }
        }
        if !config.enacted.is_empty() {
            println!("___e_id___|___actor___|___basis___|___justification___");
            for (i, e) in config.enacted.iter().enumerate() {
                println!(
                    "{: >8} | {: <9} | {:?} | {:?}",
                    i,
                    e.id.0,
                    e.basis.at,
                    e.justification.iter().map(|s| s.id.1).collect::<HashSet<_>>()
                );
            }
        }

        let stdin = std::io::stdin();
        stdin.read_line(&mut buffer).expect("buffer bad");
        let trimmed = buffer.trim_end();
        if let Some(cmd) = Cmd::parse(trimmed) {
            config.apply(cmd);
        } else {
            println!("Commands:");
            println!("- say <name> <payload>");
            println!("- agree <id> <time>");
            println!("- enact <name> <time> <id>*");
            println!("- now <time>");
            println!("- inspect");
        }
        buffer.clear();
    }
}
