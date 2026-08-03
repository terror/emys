use {super::*, add::Add, init::Init, list::List, search::Search};

mod add;
mod init;
mod list;
mod search;

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
  #[command(about = "Record a shell command", alias = "a")]
  Add(Add),
  #[command(about = "Generate shell integration")]
  Init(Init),
  #[command(about = "List recent shell commands", alias = "l")]
  List(List),
  #[command(about = "Search shell commands", alias = "s")]
  Search(Search),
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
      Self::Search(search) => search.run(database),
    }
  }
}
