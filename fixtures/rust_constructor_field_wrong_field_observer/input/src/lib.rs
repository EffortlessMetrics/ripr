#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Storage {
    Local,
    Our,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HirLet {
    pub name: String,
    pub storage: Storage,
}

fn lower_statement(name: String, storage: Storage) -> HirLet {
    HirLet {
        name,
        storage,
    }
}

pub fn lower_body(name: String, storage: Storage) -> HirLet {
    lower_statement(name, storage)
}

pub fn lower_ast(name: String, storage: Storage) -> HirLet {
    lower_body(name, storage)
}
