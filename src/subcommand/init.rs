use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Init {
  shell: Shell,
}

impl Init {
  pub(crate) fn run(self, _database: &Database) {
    print!("{}", self.shell.init());
  }
}
