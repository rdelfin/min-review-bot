use codeowners::{Owner, Owners};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::{self, Display},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnersConditional {
    And(Vec<OwnersConditional>),
    Or(Vec<OwnersConditional>),
    Owner(String),
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
            if owners.is_empty() {
                continue;
            }

            let new_item = if owners.len() == 1 {
                OwnersConditional::Owner(format!("{}", owners[0]))
            } else {
                OwnersConditional::Or(
                    owners
                        .iter()
                        .map(|owner| OwnersConditional::Owner(format!("{owner}")))
                        .collect(),
                )
            };
            // Top level ownership is always an `And`
            min_owners = match min_owners {
                OwnersConditional::And(mut and_items) => {
                    and_items.push(new_item);
                    OwnersConditional::And(and_items)
                }
                _ => {
                    unreachable!()
                }
            };
        }

        min_owners
    }

    pub fn remove_all(self, excluded_owners: &HashSet<String>) -> Option<OwnersConditional> {
        match self {
            OwnersConditional::And(owners) => {
                let new_values: Vec<_> = owners
                    .into_iter()
                    .filter_map(|cond| cond.remove_all(excluded_owners))
                    .collect();
                match new_values.len() {
                    0 => None,
                    1 => Some(new_values[0].clone()),
                    _ => Some(OwnersConditional::And(new_values)),
                }
            }
            OwnersConditional::Or(owners) => {
                let new_values: Vec<_> = owners
                    .into_iter()
                    .filter_map(|cond| cond.remove_all(excluded_owners))
                    .collect();
                match new_values.len() {
                    0 => None,
                    1 => Some(new_values[0].clone()),
                    _ => Some(OwnersConditional::Or(new_values)),
                }
            }
            OwnersConditional::Owner(owner) => {
                if excluded_owners.contains(&owner) {
                    None
                } else {
                    Some(OwnersConditional::Owner(owner))
                }
            }
        }
    }

    pub fn reduce(self) -> OwnersConditional {
        self.reduce_duplicates().reduce_or_duplicates()
    }

    fn reduce_duplicates(self) -> OwnersConditional {
        match self {
            OwnersConditional::And(items) => {
                // Remove all duplicate items
                let mut new_items = items;
                for i in (0..new_items.len()).rev() {
                    for j in 0..i {
                        if new_items[j] == new_items[i] {
                            new_items.remove(i);
                            break;
                        }
                    }
                }
                OwnersConditional::And(new_items)
            }
            OwnersConditional::Or(items) => OwnersConditional::Or(items),
            OwnersConditional::Owner(owner) => OwnersConditional::Owner(owner),
        }
    }

    fn reduce_or_duplicates(self) -> OwnersConditional {
        match self {
            OwnersConditional::And(items) => {
                // Remove all duplicate items
                let mut new_items = items;
                for i in (0..new_items.len()).rev() {
                    for j in 0..i {
                        let remove = match (
                            new_items.get(i).expect("index exists"),
                            new_items.get(j).expect("index exists"),
                        ) {
                            (OwnersConditional::Or(items), OwnersConditional::Owner(owner)) => {
                                items.contains(&OwnersConditional::Owner(owner.clone()))
                            }
                            (OwnersConditional::Owner(owner), OwnersConditional::Or(items)) => {
                                items.contains(&OwnersConditional::Owner(owner.clone()))
                            }
                            _ => false,
                        };

                        if remove {
                            new_items.remove(i);
                            break;
                        }
                    }
                }
                OwnersConditional::And(new_items)
            }
            OwnersConditional::Or(items) => OwnersConditional::Or(items),
            OwnersConditional::Owner(owner) => OwnersConditional::Owner(owner),
        }
    }
}

impl Display for OwnersConditional {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            OwnersConditional::And(items) => format!(
                "({})",
                items
                    .iter()
                    .map(|item| format!("{item}"))
                    .collect::<Vec<_>>()
                    .join(" && ")
            ),
            OwnersConditional::Or(items) => format!(
                "({})",
                items
                    .iter()
                    .map(|item| format!("{item}"))
                    .collect::<Vec<_>>()
                    .join(" || ")
            ),
            OwnersConditional::Owner(name) => name.clone(),
        };
        write!(f, "{text}")
    }
}

pub fn to_owners_map<'f, 'c>(
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

#[cfg(test)]
mod test {
    use super::OwnersConditional;
    use codeowners::Owner;
    use std::collections::HashSet;

    #[test]
    fn test_from_owners() -> anyhow::Result<()> {
        let owners_1 = vec![
            Owner::Username("owner_a".into()),
            Owner::Username("owner_b".into()),
        ];
        let owners_2 = vec![Owner::Username("owner_c".into())];
        let owners_3 = vec![Owner::Username("owner_d".into())];
        let owners_4 = vec![Owner::Username("owner_a".into())];

        assert_eq!(
            OwnersConditional::from_owners_map(
                [
                    ("a/file", Some(&owners_1)),
                    ("b/file", Some(&owners_2)),
                    ("d/file", Some(&owners_3)),
                    ("e/file", Some(&owners_4)),
                    ("w/file", None),
                ]
                .into_iter()
                .collect(),
            ),
            OwnersConditional::And(vec![
                OwnersConditional::Or(vec![
                    OwnersConditional::Owner("owner_a".into()),
                    OwnersConditional::Owner("owner_b".into())
                ]),
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_d".into()),
                OwnersConditional::Owner("owner_a".into()),
            ])
        );

        Ok(())
    }

    #[test]
    fn test_reduce() -> anyhow::Result<()> {
        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_d".into()),
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_e".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
            .reduce(),
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_d".into()),
                OwnersConditional::Owner("owner_e".into()),
            ])
        );

        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_a".into()),
                OwnersConditional::Or(vec![
                    OwnersConditional::Owner("owner_c".into()),
                    OwnersConditional::Owner("owner_e".into())
                ]),
            ])
            .reduce(),
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_c".into()),
                OwnersConditional::Owner("owner_a".into())
            ])
        );

        Ok(())
    }

    #[test]
    fn test_remove() -> anyhow::Result<()> {
        let exclude_owners: HashSet<String> = ["owner_a".into(), "owner_d".into()].into();
        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_a".into()),
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
            .remove_all(&exclude_owners)
            .ok_or_else(|| anyhow::anyhow!("remove_all gave empty owners"))?
            .reduce(),
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
        );

        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Or(vec![
                    OwnersConditional::Owner("owner_a".into()),
                    OwnersConditional::Owner("owner_d".into()),
                ]),
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
            .remove_all(&exclude_owners)
            .ok_or_else(|| anyhow::anyhow!("remove_all gave empty owners"))?
            .reduce(),
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
        );

        assert_eq!(
            OwnersConditional::Or(vec![
                OwnersConditional::And(vec![
                    OwnersConditional::Owner("owner_a".into()),
                    OwnersConditional::Owner("owner_d".into()),
                ]),
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
            .remove_all(&exclude_owners)
            .ok_or_else(|| anyhow::anyhow!("remove_all gave empty owners"))?
            .reduce(),
            OwnersConditional::Or(vec![
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
        );

        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Or(vec![
                    OwnersConditional::Owner("owner_a".into()),
                    OwnersConditional::Owner("owner_e".into()),
                ]),
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
            .remove_all(&exclude_owners)
            .ok_or_else(|| anyhow::anyhow!("remove_all gave empty owners"))?
            .reduce(),
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_e".into()),
                OwnersConditional::Owner("owner_b".into()),
                OwnersConditional::Owner("owner_c".into()),
            ])
        );

        assert_eq!(
            OwnersConditional::And(vec![
                OwnersConditional::Owner("owner_a".into()),
                OwnersConditional::Owner("owner_d".into()),
            ])
            .remove_all(&exclude_owners),
            None,
        );

        Ok(())
    }
}
