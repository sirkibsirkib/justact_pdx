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

enum UpdateCmd<'a> {
    Say { sayer: &'a str, payload: &'a str },
    Agree { on_idx: StmtIdx, at: Time },
    Enact { actor: &'a str, basis: AgreeIdx, justification: HashSet<StmtIdx> },
    Now { now: Time },
}

enum Cmd<'a> {
    Update(UpdateCmd<'a>),
    Inspect,
    Quit,
    Show,
    Dump,
}

impl<'a> Cmd<'a> {
    fn parse(input: &'a str) -> Option<Self> {
        let mut splits = input.splitn(3, char::is_whitespace);
        let keyword = splits.next()?;
        use Cmd::*;
        use UpdateCmd::*;
        match keyword {
            "say" => {
                let sayer = splits.next()?;
                let payload = splits.next()?;
                Some(Update(Say { sayer, payload }))
            }
            "agree" => {
                let on_idx: StmtIdx = splits.next()?.parse().ok()?;
                let at: Time = splits.next()?.parse().ok()?;
                Some(Update(Agree { on_idx, at }))
            }
            "enact" => {
                let actor = splits.next()?;
                let rest = splits.next()?;
                let mut splits = rest.split(char::is_whitespace);
                let basis: AgreeIdx = splits.next()?.parse().ok()?;
                let justification: HashSet<StmtIdx> =
                    splits.map(|part| part.parse().ok()).collect::<Option<_>>()?;
                Some(Update(Enact { actor, basis, justification }))
            }
            "now" => {
                let now: Time = splits.next()?.parse().ok()?;
                if splits.all(str::is_empty) {
                    Some(Update(Now { now }))
                } else {
                    None
                }
            }
            "inspect" => Some(Inspect),
            "quit" => Some(Quit),
            "dump" => Some(Dump),
            "show" => Some(Show),
            _ => None,
        }
    }
}

impl Config {
    fn update(&mut self, update_cmd: UpdateCmd) {
        match update_cmd {
            UpdateCmd::Say { sayer, payload } => self.statements.push(Arc::new(Message {
                id: (sayer.to_string(), self.statements.len().try_into().unwrap()),
                payload: payload.to_string(),
            })),
            UpdateCmd::Agree { on_idx, at } => {
                if let Some(s) = self.statements.get_mut(on_idx) {
                    self.agreements.push(Agreement { at, message: s.clone() });
                } else {
                    println!("Limitation: cannot agree on unsaid messages!");
                }
            }
            UpdateCmd::Enact { actor, basis, justification } => {
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
            UpdateCmd::Now { now } => {
                self.current = now;
            }
        }
    }

    fn write_inspection<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
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
            writeln!(w, "{}", serde_json::to_string(&Event::Control(c)).expect("WAH"))?;
        }
        Ok(())
    }

    fn run_inspection(&self) -> std::io::Result<()> {
        use std::process::{Command, Stdio};
        let mut child = Command::new("./inspector.exe").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            self.write_inspection(&mut stdin)?;
        }
        child.wait()?;
        println!("ok, let's continue");
        Ok(())
    }

    fn dump(&self) -> std::io::Result<()> {
        self.write_inspection(std::io::stdout().lock())
    }

    fn show(&self) {
        println!("current time: {}", self.current);
        if !self.statements.is_empty() {
            println!("__stmt.id__|___sayer___|___payload___ STATEMENTS");
            for (i, s) in self.statements.iter().enumerate() {
                let [a, b] = trucated(&s.payload);
                println!("{: >8} | {: <9} | {:?}{}", i, s.id.0, a, b);
            }
        }
        if !self.agreements.is_empty() {
            println!("___ag.id___|___s_id___|___time___ AGREEMENTS");
            for (i, a) in self.agreements.iter().enumerate() {
                println!("{: >8} | {: <9} | {:?}", i, a.message.id.1, a.at);
            }
        }
        if !self.enacted.is_empty() {
            println!("___act.id__|___actor___|___basis___|___justification___ ENACTED ACTIONS");
            for (i, e) in self.enacted.iter().enumerate() {
                println!(
                    "{: >8} | {: <9} | {:?} | {:?}",
                    i,
                    e.id.0,
                    e.basis.at,
                    e.justification.iter().map(|s| s.id.1).collect::<HashSet<_>>()
                );
            }
        }
    }
}

fn trucated(s: &str) -> [&str; 2] {
    const MAX_BYTES: usize = 40;
    if let Some(cutoff) = s.char_indices().nth(MAX_BYTES).map(|(idx, _)| idx) {
        [&s[..cutoff], "..."]
    } else {
        [s, ""]
    }
}

fn main() {
    let mut config = Config { current: 0, statements: vec![], agreements: vec![], enacted: vec![] };
    let mut buffer = String::new();
    'outer: loop {
        let stdin = std::io::stdin();
        stdin.read_line(&mut buffer).expect("buffer bad");
        if buffer.is_empty() {
            // reading line ended NOT because it reached the end of the line
            break 'outer;
        }
        let trimmed = buffer.trim_end();
        if let Some(cmd) = Cmd::parse(trimmed) {
            match cmd {
                Cmd::Update(update_cmd) => config.update(update_cmd),
                Cmd::Quit => break 'outer,
                Cmd::Inspect => config.run_inspection().expect("inspect bad"),
                Cmd::Dump => config.dump().expect("dump bad"),
                Cmd::Show => config.show(),
            }
        } else {
            println!("Commands:");
            println!("- say <name> <payload>");
            println!("- agree <stmt.id> <time>");
            println!("- enact <name> <ag.id> <stmt.id>*");
            println!("- now <time>");
            println!("- inspect");
            println!("- show");
            println!("- dump");
            println!("- quit")
        }
        buffer.clear();
    }
}

// (cat example.txt & cat) | .\target\release\justact-pdx.exe
