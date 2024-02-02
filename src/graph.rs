use crate::core::{Event, Signal, StrRange};
use petgraph::graph::{DiGraph, NodeIndex};
use std::{
    collections::{hash_map, HashMap},
    mem,
    ops::Range,
};

struct Choice<'a> {
    from_anchor: NodeIndex,
    to_anchor: &'a str,
    range: Range<usize>,
}

// NOTE: can add `2` to  signal params ends and sub `1` from signal prompt starts,
//       because braces and signal chars in `texal` are always ascii
fn node_pass<'a>(
    range_graph: &mut DiGraph<Range<usize>, Range<usize>>,
    bookmark_map: &mut HashMap<&'a str, NodeIndex>,
    choice_map: &mut Vec<Choice<'a>>,
    iter: impl IntoIterator<Item = Event<'a>>,
) {
    let mut current_end = 0;
    let mut last_bookmark_index = NodeIndex::default();
    let mut unclosed_param = None;
    let mut is_prev_bookmark = false;
    for event in iter {
        match event {
            Event::Signal(Signal::Call {
                prompt:
                    StrRange {
                        slice: next_prompt_slice @ ("bookmark" | "choice"),
                        ..
                    },
                param,
            }) if unclosed_param.is_none() => {
                if next_prompt_slice == "bookmark" {
                    unclosed_param = Some(param);
                    is_prev_bookmark = true;
                }
            }
            Event::Signal(Signal::Call {
                prompt:
                    StrRange {
                        slice: next_prompt_slice @ ("bookmark" | "choice"),
                        range: next_prompt_range,
                    },
                param: next_param,
            }) => {
                let prev_param = unclosed_param.replace(next_param.clone()).unwrap();
                if mem::replace(&mut is_prev_bookmark, next_prompt_slice == "bookmark") {
                    match bookmark_map.entry(prev_param.slice) {
                        hash_map::Entry::Occupied(_) => (),
                        hash_map::Entry::Vacant(anchor_entry) => {
                            last_bookmark_index = range_graph
                                .add_node(prev_param.range.end + 1..next_prompt_range.start - 1);
                            anchor_entry.insert(last_bookmark_index);
                        }
                    }
                } else {
                    choice_map.push(Choice {
                        from_anchor: last_bookmark_index,
                        to_anchor: prev_param.slice,
                        range: prev_param.range.end + 1..next_prompt_range.start - 1,
                    });
                }
            }
            Event::Signal(
                Signal::Call {
                    param: StrRange { range, .. },
                    ..
                }
                | Signal::Param(StrRange { range, .. }),
            ) => current_end = range.end + 1,
            Event::Signal(Signal::Prompt(StrRange { range, .. }))
            | Event::Text(StrRange { range, .. }) => {
                current_end = range.end;
            }
            _ => (),
        }
    }
    if let Some(prev_param) = unclosed_param {
        if is_prev_bookmark {
            match bookmark_map.entry(prev_param.slice) {
                hash_map::Entry::Occupied(_) => (),
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(range_graph.add_node(prev_param.range.end + 1..current_end));
                }
            }
        } else {
            choice_map.push(Choice {
                from_anchor: last_bookmark_index,
                to_anchor: prev_param.slice,
                range: prev_param.range.end + 1..current_end,
            });
        }
    }
}

fn edge_pass<'a>(
    range_graph: &mut DiGraph<Range<usize>, Range<usize>>,
    anchor_map: &HashMap<&'a str, NodeIndex>,
    choice_map: &[Choice<'a>],
) {
    for choice in choice_map {
        if let Some(to_anchor_index) = anchor_map.get(choice.to_anchor) {
            range_graph.add_edge(choice.from_anchor, *to_anchor_index, choice.range.clone());
        }
    }
}

/// Guide can help searching for the particular bookmark story should continue from
pub type Guide<'a> = HashMap<&'a str, NodeIndex>;

/// A story is a graph where spans of text are connected to each other through choices.
/// Ranges of original string stored in nodes relate to main text under a particular `bookmark`,
/// and the ranges stored in edges relate to the text of a certain `choice`.
pub type Story = DiGraph<Range<usize>, Range<usize>>;

fn from_iter<'a, I: IntoIterator<Item = Event<'a>>>(iter: I) -> (Guide<'a>, Story) {
    let mut range_graph = DiGraph::new();
    let mut anchor_map = HashMap::new();
    let mut choice_map = Vec::new();
    node_pass(&mut range_graph, &mut anchor_map, &mut choice_map, iter);
    edge_pass(&mut range_graph, &anchor_map, &choice_map);
    (anchor_map, range_graph)
}

/// Consume `bookmark` and `choice` signals from text to create a graph
#[must_use]
pub fn read<'a, I: IntoIterator<Item = &'a str>>(text_chunks: I) -> (Guide<'a>, Story) {
    from_iter(text_chunks.into_iter().flat_map(crate::core::Iter::new))
}

#[cfg(test)]
mod tests {
    #[test]
    fn single_bookmark() {
        const SAMPLE: &str = "@bookmark{greet}Hello, World!";
        let (guide, story) = super::from_iter(crate::core::Iter::new(SAMPLE));
        assert_eq!(guide.len(), 1);
        assert_eq!(story.node_count(), 1);
        assert_eq!(story.edge_count(), 0);
        let bookmark_index = guide.get("greet").expect("greet");
        let text_range = story[*bookmark_index].clone();
        assert_eq!(&SAMPLE[text_range], "Hello, World!");
    }

    #[test]
    fn double_bookmark() {
        const SAMPLE: &str =
            "@bookmark{greet}Hello, World!\n@bookmark{greet-back}Hello back at you!";
        let (guide, story) = super::from_iter(crate::core::Iter::new(SAMPLE));
        assert_eq!(guide.len(), 2);
        assert_eq!(story.node_count(), 2);
        assert_eq!(story.edge_count(), 0);
        let bookmark_index = guide.get("greet").expect("greet");
        let text_range = story[*bookmark_index].clone();
        assert_eq!(&SAMPLE[text_range], "Hello, World!\n");
        let bookmark_index = guide.get("greet-back").expect("greet-back");
        let text_range = story[*bookmark_index].clone();
        assert_eq!(&SAMPLE[text_range], "Hello back at you!");
    }

    #[test]
    fn choices() {
        const SAMPLE: &str = "@bookmark{greet}Hello, World!\n@choice{end}Hi!\n@choice{end}Hello back at you!\n@bookmark{end}End.";
        let (guide, story) = super::from_iter(crate::core::Iter::new(SAMPLE));
        assert_eq!(guide.len(), 2);
        assert_eq!(story.node_count(), 2);
        assert_eq!(story.edge_count(), 2);
        let greet_index = guide.get("greet").expect("greet");
        let text_range = story[*greet_index].clone();
        assert_eq!(&SAMPLE[text_range], "Hello, World!\n");
        let end_index = guide.get("end").expect("end");
        let text_range = story[*end_index].clone();
        assert_eq!(&SAMPLE[text_range], "End.");
        let mut edges = story.edges_connecting(*greet_index, *end_index);
        let hello_back_edge = edges.next().unwrap();
        assert_eq!(
            &SAMPLE[hello_back_edge.weight().clone()],
            "Hello back at you!\n"
        );
        let hi_edge = edges.next().unwrap();
        assert_eq!(&SAMPLE[hi_edge.weight().clone()], "Hi!\n");
    }
}
