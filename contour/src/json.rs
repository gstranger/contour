use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::{Graph, model::{Color, FillState, Vec2, HandleMode}};

pub fn to_json_impl(g: &Graph) -> Value {
    #[derive(Serialize)] struct NodeSer{ id:u32,x:f32,y:f32 }
    #[derive(Serialize)] #[serde(tag="kind", rename_all="lowercase")]
    enum EdgeSerKind{ Line, Cubic{ ha:Vec2, hb:Vec2, mode:HandleMode }, Polyline{ points:Vec<Vec2> } }
    #[derive(Serialize)] struct EdgeSer{ id:u32,a:u32,b:u32, #[serde(flatten)] kind:EdgeSerKind, stroke:Option<Color>, width:f32 }
    #[derive(Serialize)] struct FillSer{ key:u32, filled:bool, color:Option<Color> }
    #[derive(Serialize)] struct Doc{ version:u32, nodes:Vec<NodeSer>, edges:Vec<EdgeSer>, fills:Vec<FillSer> }
    let mut nodes=Vec::new(); for (i,n) in g.nodes.iter().enumerate() { if let Some(n)=n { nodes.push(NodeSer{id:i as u32,x:n.x,y:n.y}); } }
    let mut edges=Vec::new(); for (i,e) in g.edges.iter().enumerate() { if let Some(e)=e { let kind=match &e.kind { crate::model::EdgeKind::Line=>EdgeSerKind::Line, crate::model::EdgeKind::Cubic{ha,hb,mode}=>EdgeSerKind::Cubic{ha:*ha,hb:*hb,mode:*mode}, crate::model::EdgeKind::Polyline{points}=>EdgeSerKind::Polyline{ points:points.clone() } }; edges.push(EdgeSer{id:i as u32,a:e.a,b:e.b,kind,stroke:e.stroke,width:e.stroke_width}); } }
    let mut fills=Vec::new(); for (k,v) in g.fills.iter() { fills.push(FillSer{ key:*k, filled:v.filled, color:v.color }); }
    serde_json::to_value(Doc{ version:1, nodes, edges, fills }).unwrap()
}

pub fn from_json_impl(g: &mut Graph, v: Value) -> bool {
    #[derive(Deserialize)] struct NodeDe{ id:u32,x:f32,y:f32 }
    #[derive(Deserialize)] #[serde(tag="kind", rename_all="lowercase")]
    enum EdgeDeKind{ Line, Cubic{ ha:Vec2, hb:Vec2, mode:Option<HandleMode> }, Polyline{ points:Vec<Vec2> } }
    #[derive(Deserialize)] struct EdgeDe{ id:u32,a:u32,b:u32, #[serde(flatten)] kind:Option<EdgeDeKind>, stroke:Option<Color>, width:Option<f32> }
    #[derive(Deserialize)] struct FillDe{ key:u32, filled:bool, color:Option<Color> }
    #[derive(Deserialize)] struct DocDe{ version:Option<u32>, nodes:Vec<NodeDe>, edges:Vec<EdgeDe>, fills:Option<Vec<FillDe>> }
    let parsed:Result<DocDe,_>=serde_json::from_value(v); if let Ok(doc)=parsed {
        let max_node=doc.nodes.iter().map(|n| n.id).max().unwrap_or(0);
        let max_edge=doc.edges.iter().map(|e| e.id).max().unwrap_or(0);
        g.nodes=vec![None; (max_node as usize)+1]; g.edges=vec![None; (max_edge as usize)+1]; g.fills.clear();
        for n in doc.nodes { g.nodes[n.id as usize]=Some(crate::model::Node{ x:n.x, y:n.y }); }
        for e in doc.edges {
            let a_ok = g.nodes.get(e.a as usize).and_then(|n| *n).is_some();
            let b_ok = g.nodes.get(e.b as usize).and_then(|n| *n).is_some();
            if !a_ok || !b_ok { continue; }
            let kind=match e.kind.unwrap_or(EdgeDeKind::Line) {
                EdgeDeKind::Line=>crate::model::EdgeKind::Line,
                EdgeDeKind::Cubic{ha,hb,mode}=>crate::model::EdgeKind::Cubic{ha,hb,mode:mode.unwrap_or(HandleMode::Free)},
                EdgeDeKind::Polyline{points}=>crate::model::EdgeKind::Polyline{ points }
            };
            g.edges[e.id as usize]=Some(crate::model::Edge{ a:e.a,b:e.b,kind,stroke:e.stroke,stroke_width:e.width.unwrap_or(2.0) });
        }
        if let Some(fills)=doc.fills { for f in fills { g.fills.insert(f.key, FillState{filled:f.filled,color:f.color}); } }
        g.geom_ver=g.geom_ver.wrapping_add(1); true
    } else { false }
}
