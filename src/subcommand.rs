use {super::*, add::Add, list::List};

mod add;
mod list;

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
  #[command(about = "Record a shell command", alias = "a")]
  Add(Add),
  #[command(about = "List recent shell commands", alias = "l")]
  List(List),
}

impl Subcommand {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Add(add) => add.run(database),
      Self::List(list) => list.run(database),
    }
  }
}
