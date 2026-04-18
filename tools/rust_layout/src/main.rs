use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use taffy::prelude::*;
use taffy::TaffyTree;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockStyle {
    display: String,
    flex_direction: String,
    width: Option<f32>,
    height: Option<f32>,
    flex_grow: f32,
    padding: [f32; 4], // top, right, bottom, left
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockLayoutNode {
    id: u32,
    style: MockStyle,
    children: Vec<MockLayoutNode>,
}

fn map_style(mock: &MockStyle) -> Style {
    let mut style = Style::default();
    
    style.display = match mock.display.as_str() {
        "flex" => Display::Flex,
        "none" => Display::None,
        _ => Display::Block,
    };

    style.flex_direction = match mock.flex_direction.as_str() {
        "column" => FlexDirection::Column,
        _ => FlexDirection::Row,
    };

    if let Some(w) = mock.width {
        style.size.width = length(w);
    }
    if let Some(h) = mock.height {
        style.size.height = length(h);
    }

    style.flex_grow = mock.flex_grow;
    style.padding = Rect {
        left: length(mock.padding[3]),
        right: length(mock.padding[1]),
        top: length(mock.padding[0]),
        bottom: length(mock.padding[2]),
    };

    style
}

fn build_tree(taffy: &mut TaffyTree<()>, mock: &MockLayoutNode) -> NodeId {
    let children: Vec<NodeId> = mock.children.iter().map(|c| build_tree(taffy, c)).collect();
    let style = map_style(&mock.style);
    taffy.new_with_children(style, &children).unwrap()
}

fn print_layout(taffy: &TaffyTree<()>, node: NodeId, mock: &MockLayoutNode) {
    let layout = taffy.layout(node).unwrap();
    println!("NODE {} X={} Y={} W={} H={}", mock.id, layout.location.x, layout.location.y, layout.size.width, layout.size.height);
    
    let children = taffy.children(node).unwrap();
    for (i, child) in children.iter().enumerate() {
        print_layout(taffy, *child, &mock.children[i]);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <layout_json>", args[0]);
        std::process::exit(1);
    }

    let json_content = fs::read_to_string(&args[1]).expect("Failed to read JSON");
    let root_mock: MockLayoutNode = serde_json::from_str(&json_content).expect("Failed to parse JSON");

    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let root_node = build_tree(&mut taffy, &root_mock);
    
    taffy.compute_layout(root_node, Size::MAX_CONTENT).unwrap();

    println!("== LAYOUT IR ==");
    print_layout(&taffy, root_node, &root_mock);
}
