use {super::*, add::Add, init::Init, list::List};

mod add;
mod init;
mod list;

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
  #[command(about = "Record a shell command", alias = "a")]
  Add(Add),
  #[command(about = "Generate shell integration")]
  Init(Init),
  #[command(about = "List recent shell commands", alias = "l")]
  List(List),
}

impl Subcommand {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Add(add) => add.run(database),
      Self::Init(init) => {
        init.run(database);
        Ok(())
      }
      Self::List(list) => list.run(database),
    }
  }
}
