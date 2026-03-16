// SiteOne Crawler - Option type definitions
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionType {
    Int,
    Float,
    Bool,
    String,
    SizeMG,
    Email,
    Url,
    Regex,
    File,
    Dir,
    HostAndPort,
    ReplaceContent,
    Resolve,
}

impl fmt::Display for OptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptionType::Int => write!(f, "INT"),
            OptionType::Float => write!(f, "FLOAT"),
            OptionType::Bool => write!(f, "BOOL"),
            OptionType::String => write!(f, "STRING"),
            OptionType::SizeMG => write!(f, "SIZE_M_G"),
            OptionType::Email => write!(f, "EMAIL"),
            OptionType::Url => write!(f, "URL"),
            OptionType::Regex => write!(f, "REGEX"),
            OptionType::File => write!(f, "FILE"),
            OptionType::Dir => write!(f, "DIR"),
            OptionType::HostAndPort => write!(f, "HOST_AND_PORT"),
            OptionType::ReplaceContent => write!(f, "REPLACE_CONTENT"),
            OptionType::Resolve => write!(f, "RESOLVE"),
        }
    }
}
