use codeowners::{Owner, Owners};
use std::{
    collections::BTreeMap,
    fmt::{self, Display},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnersConditional {
    And(Vec<OwnersItem>),
    Or(Vec<OwnersItem>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnersItem {
    Owner(String),
    Conditional(OwnersConditional),
}

impl OwnersConditional {
    pub fn from_codeowners(codeowners: &Owners, files: &[&str]) -> OwnersConditional {
        let owners_map = to_owners_map(codeowners, files);
        OwnersConditional::from_owners_map(owners_map)
    }

    fn from_owners_map(owners_map: BTreeMap<&str, Option<&Vec<Owner>>>) -> OwnersConditional {
        let mut min_owners = OwnersConditional::And(vec![]);

        for (_, owners) in owners_map.into_iter() {
            let owners = match owners {
                Some(owners) => owners,
                None => {
                    continue;
                }
            };
            if owners.len() == 0 {
                continue;
            }

            let new_item = if owners.len() == 1 {
                OwnersItem::Owner(format!("{}", owners[0]))
            } else {
                OwnersItem::Conditional(OwnersConditional::Or(
                    owners
                        .iter()
                        .map(|owner| OwnersItem::Owner(format!("{owner}")))
                        .collect(),
                ))
            };
            min_owners = match min_owners {
                OwnersConditional::And(mut and_items) => {
                    and_items.push(new_item);
                    OwnersConditional::And(and_items)
                }
                OwnersConditional::Or(_) => {
                    unreachable!()
                }
            };
        }

        min_owners
    }

    pub fn reduce(&mut self) {
        match self {
            OwnersConditional::And(items) => {
                // Remove duplicate items
                let mut to_remove = vec![];
                for i in 0..(items.len() - 1) {
                    for j in (i + 1)..items.len() {
                        if items[i] == items[j] {
                            to_remove.push(j);
                        }
                    }
                }
                for item in to_remove.iter().rev() {
                    items.remove(*item);
                }
            }
            OwnersConditional::Or(_) => {}
        }
    }
}

impl Display for OwnersConditional {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            OwnersConditional::And(items) => items
                .iter()
                .map(|item| format!("{item}"))
                .collect::<Vec<_>>()
                .join(" && "),
            OwnersConditional::Or(items) => items
                .iter()
                .map(|item| format!("{item}"))
                .collect::<Vec<_>>()
                .join(" || "),
        };
        write!(f, "{text}")
    }
}

impl Display for OwnersItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OwnersItem::Owner(name) => {
                write!(f, "{name}")
            }
            OwnersItem::Conditional(cond) => {
                write!(f, "({cond})")
            }
        }
    }
}

fn to_owners_map<'f, 'c>(
    codeowners: &'c Owners,
    files: &[&'f str],
) -> BTreeMap<&'f str, Option<&'c Vec<Owner>>> {
    files
        .iter()
        .map(|file| {
            let owners = codeowners.of(*file);
            (*file, owners)
        })
        .collect()
}
