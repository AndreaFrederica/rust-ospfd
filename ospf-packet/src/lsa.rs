#![allow(unused)]

#[derive(Clone, Debug)]
pub struct LSAHeader {
    //todo!
}

#[derive(Clone, Debug)]
pub enum LSA {
    Router(RouterLSA),
    Network(NetworkLSA),
    SummaryIP(SummaryIpLSA),
    SummaryASBR(SummaryAsbrLSA),
    ASExternal(AsExternalLSA),
}

#[derive(Clone, Debug)]
pub struct RouterLSA {
    //todo!
}

#[derive(Clone, Debug)]
pub struct NetworkLSA {
    //todo!
}

#[derive(Clone, Debug)]
pub struct SummaryIpLSA {
    //todo!
}

#[derive(Clone, Debug)]
pub struct SummaryAsbrLSA {
    //todo!
}

#[derive(Clone, Debug)]
pub struct AsExternalLSA {
    //todo!
}
