mod behavior;
mod fixture;
mod roots;
mod workers;

fn scope() -> cyanrex_engine::models::collaboration::WorkspaceRef {
    cyanrex_engine::models::collaboration::WorkspaceRef {
        authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
    }
}
