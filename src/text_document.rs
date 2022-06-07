pub struct TextDocument {}

pub struct TextDocumentOption {
    pub tabs: Vec<Tab>,
}

pub struct Tab {
    pub position: usize,
    pub tab_type: TabType,
    pub delimiter: char,
}

pub enum TabType {
    LeftTab,RightTab, CenterTab, DelimiterTab
}