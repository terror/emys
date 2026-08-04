use {
  super::*, add::Add, backup::Backup, r#import::Import, init::Init, list::List,
  search::Search,
};

mod add;
mod backup;
mod clear;
mod r#import;
mod init;
mod list;
mod search;

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
  #[command(about = "Record a shell command", alias = "a")]
  Add(Add),
  #[command(about = "Back up the database")]
  Backup(Backup),
  #[command(about = "Clear the history database")]
  Clear,
  #[command(about = "Import shell history", alias = "i", subcommand)]
  Import(Import),
  #[command(about = "Generate shell integration")]
  Init(Init),
  #[command(about = "List recent shell commands", alias = "l")]
  List(List),
  #[command(about = "Search shell commands", alias = "s")]
  Search(Search),
}

impl Subcommand {
  pub(crate) fn run(self, database: Database) -> Result {
    match self {
      Self::Add(add) => add.run(&database),
      Self::Backup(backup) => backup.run(&database),
      Self::Clear => clear::run(&database),
      Self::Import(import) => import.run(&database),
      Self::Init(init) => {
        init.run(&database);
        Ok(())
      }
      Self::List(list) => list.run(&database),
      Self::Search(search) => search.run(database),
    }
  }
}
