//! Árbol binario de paneles.
//!
//! Un split tiene eje, dos hijos y un ratio en `(0, 1)`. El ratio vive en el
//! split, nunca en el panel (ADR 0003). Las hojas son `PaneId`.

use serde::{Deserialize, Serialize};

use crate::PaneId;

/// Eje de división de un split.
///
/// `Horizontal` coloca el primer hijo a la izquierda y el segundo a la derecha
/// (el divisor es vertical). `Vertical` los coloca arriba y abajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SplitAxis {
    /// Primero a la izquierda, segundo a la derecha.
    Horizontal,
    /// Primero arriba, segundo abajo.
    Vertical,
}

/// Porción del camino desde la raíz hasta un split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Branch {
    /// Hijo primero (izquierda o arriba).
    First,
    /// Hijo segundo (derecha o abajo).
    Second,
}

/// Ratio mínimo de cada lado de un split (ADR 0003). La app puede ser más
/// estricta según las métricas, pero el modelo nunca permite un lado vacío.
pub const MIN_RATIO: f32 = 0.1;

/// Árbol binario de paneles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum PaneTree {
    /// División en dos subárboles.
    Split {
        /// Eje de la división.
        axis: SplitAxis,
        /// Proporción asignada al primer hijo.
        ratio: f32,
        /// Hijo primero.
        first: Box<PaneTree>,
        /// Hijo segundo.
        second: Box<PaneTree>,
    },
    /// Panel hoja.
    Leaf(PaneId),
}

impl PaneTree {
    /// Árbol con un único panel.
    pub fn leaf(pane: PaneId) -> Self {
        Self::Leaf(pane)
    }

    /// `true` si el árbol es una única hoja.
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }

    /// Panel de la hoja raíz, si lo es.
    pub fn leaf_id(&self) -> Option<PaneId> {
        match self {
            Self::Leaf(id) => Some(*id),
            Self::Split { .. } => None,
        }
    }

    /// Hojas en orden de lectura (izquierda/derecha, arriba/abajo).
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut leaves = Vec::new();
        self.collect_leaves(&mut leaves);
        leaves
    }

    fn collect_leaves(&self, leaves: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(id) => leaves.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_leaves(leaves);
                second.collect_leaves(leaves);
            }
        }
    }

    /// `true` si el panel es una hoja del árbol.
    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    /// Camino desde la raíz hasta la hoja `pane`.
    pub fn path_to(&self, pane: PaneId) -> Option<Vec<Branch>> {
        let mut path = Vec::new();
        if self.find_path(pane, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn find_path(&self, pane: PaneId, path: &mut Vec<Branch>) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => {
                path.push(Branch::First);
                if first.find_path(pane, path) {
                    return true;
                }
                path.pop();
                path.push(Branch::Second);
                if second.find_path(pane, path) {
                    return true;
                }
                path.pop();
                false
            }
        }
    }

    /// Reemplaza la hoja `pane` por un split con ella misma y `new_pane`.
    ///
    /// `new_side` decide en qué lado del split queda `new_pane`. `ratio` es la
    /// proporción del primer hijo, igual que en [`Self::Split`].
    ///
    /// Devuelve `false` si `pane` no está en el árbol.
    pub fn split_with_side(
        &mut self,
        pane: PaneId,
        new_pane: PaneId,
        axis: SplitAxis,
        new_side: Branch,
        ratio: f32,
    ) -> bool {
        if pane == new_pane {
            return false;
        }
        if let Self::Leaf(id) = self
            && *id == pane
        {
            let original = Self::Leaf(pane);
            let (first, second) = match new_side {
                Branch::First => (Self::Leaf(new_pane), original),
                Branch::Second => (original, Self::Leaf(new_pane)),
            };
            *self = Self::Split {
                axis,
                ratio: clamp_ratio(ratio),
                first: Box::new(first),
                second: Box::new(second),
            };
            return true;
        }
        match self {
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split_with_side(pane, new_pane, axis, new_side, ratio)
                    || second.split_with_side(pane, new_pane, axis, new_side, ratio)
            }
        }
    }

    /// Reemplaza la hoja `pane` por un split con ella misma y `new_pane`.
    ///
    /// `new_pane` queda a la derecha o abajo.
    ///
    /// Devuelve `false` si `pane` no está en el árbol.
    pub fn split(&mut self, pane: PaneId, new_pane: PaneId, axis: SplitAxis, ratio: f32) -> bool {
        self.split_with_side(pane, new_pane, axis, Branch::Second, ratio)
    }

    /// Colapsa la hoja `pane` sustituyendo su split padre por el hermano.
    ///
    /// No colapsa la raíz: un workspace siempre conserva al menos un panel.
    /// Devuelve `false` si `pane` no existe o es la raíz.
    pub fn collapse(&mut self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                if first.leaf_id() == Some(pane) {
                    *self = (**second).clone();
                    return true;
                }
                if second.leaf_id() == Some(pane) {
                    *self = (**first).clone();
                    return true;
                }
                first.collapse(pane) || second.collapse(pane)
            }
        }
    }

    /// Ratio del split en `path`, si existe y es un split.
    pub fn ratio_at(&self, path: &[Branch]) -> Option<f32> {
        let node = self.node_at(path)?;
        match node {
            Self::Split { ratio, .. } => Some(*ratio),
            Self::Leaf(_) => None,
        }
    }

    /// Eje del split en `path`.
    pub fn axis_at(&self, path: &[Branch]) -> Option<SplitAxis> {
        let node = self.node_at(path)?;
        match node {
            Self::Split { axis, .. } => Some(*axis),
            Self::Leaf(_) => None,
        }
    }

    /// Ajusta el ratio del split en `path`, saneándolo a `(0, 1)`.
    pub fn set_ratio_at(&mut self, path: &[Branch], ratio: f32) -> bool {
        let Some(node) = self.node_at_mut(path) else {
            return false;
        };
        match node {
            Self::Split { ratio: current, .. } => {
                *current = clamp_ratio(ratio);
                true
            }
            Self::Leaf(_) => false,
        }
    }

    fn node_at(&self, path: &[Branch]) -> Option<&Self> {
        let mut node = self;
        for branch in path {
            match (node, branch) {
                (Self::Split { first, .. }, Branch::First) => node = first,
                (Self::Split { second, .. }, Branch::Second) => node = second,
                (Self::Leaf(_), _) => return None,
            }
        }
        Some(node)
    }

    fn node_at_mut(&mut self, path: &[Branch]) -> Option<&mut Self> {
        let mut node = self;
        for branch in path {
            match (node, branch) {
                (Self::Split { first, .. }, Branch::First) => node = first,
                (Self::Split { second, .. }, Branch::Second) => node = second,
                (Self::Leaf(_), _) => return None,
            }
        }
        Some(node)
    }

    /// Sanea ratios y elimina hojas duplicadas (sesión corrupta).
    ///
    /// Devuelve `false` si el árbol no se pudo reparar y debe reiniciarse.
    pub fn repair(&mut self) -> bool {
        let mut seen = Vec::new();
        self.repair_node(&mut seen)
    }

    fn repair_node(&mut self, seen: &mut Vec<PaneId>) -> bool {
        match self {
            Self::Leaf(id) => {
                if seen.contains(id) {
                    return false;
                }
                seen.push(*id);
                true
            }
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                *ratio = clamp_ratio(*ratio);
                first.repair_node(seen) && second.repair_node(seen)
            }
        }
    }

    /// Panel vecino inmediato (hermano del split que lo contiene).
    pub fn sibling_of(&self, pane: PaneId) -> Option<PaneId> {
        match self {
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                if first.contains(pane) {
                    if first.leaf_id() == Some(pane) {
                        second.leaves().first().copied()
                    } else {
                        first.sibling_of(pane)
                    }
                } else if second.contains(pane) {
                    if second.leaf_id() == Some(pane) {
                        first.leaves().first().copied()
                    } else {
                        second.sibling_of(pane)
                    }
                } else {
                    None
                }
            }
        }
    }
}

fn clamp_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(MIN_RATIO, 1.0 - MIN_RATIO)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (PaneTree, [PaneId; 3]) {
        let a = PaneId::new(1);
        let b = PaneId::new(2);
        let c = PaneId::new(3);
        let mut tree = PaneTree::leaf(a);
        assert!(tree.split(a, b, SplitAxis::Horizontal, 0.5));
        assert!(tree.split(b, c, SplitAxis::Vertical, 2.0));
        (tree, [a, b, c])
    }

    #[test]
    fn split_replaces_leaf_and_clamps_ratio() {
        let (tree, [a, b, c]) = tree();
        assert_eq!(tree.leaves(), vec![a, b, c]);
        let path = tree.path_to(b).expect("camino a b");
        let split = &path[..path.len() - 1];
        assert_eq!(tree.ratio_at(split), Some(1.0 - MIN_RATIO));
        assert_eq!(tree.axis_at(split), Some(SplitAxis::Vertical));
    }

    #[test]
    fn split_with_side_places_the_new_pane_on_the_requested_side() {
        let a = PaneId::new(1);
        let b = PaneId::new(2);
        let c = PaneId::new(3);

        let mut second = PaneTree::leaf(a);
        assert!(second.split_with_side(a, b, SplitAxis::Horizontal, Branch::Second, 0.5));
        assert_eq!(second.leaves(), vec![a, b]);

        let mut first = PaneTree::leaf(a);
        assert!(first.split_with_side(a, c, SplitAxis::Vertical, Branch::First, 0.5));
        assert_eq!(first.leaves(), vec![c, a]);
        let path = first.path_to(c).expect("camino a c");
        assert_eq!(
            first.axis_at(&path[..path.len() - 1]),
            Some(SplitAxis::Vertical)
        );
    }

    #[test]
    fn collapse_replaces_parent_with_sibling() {
        let (mut tree, [a, _b, c]) = tree();
        // `b` es hijo del split interno: su padre se sustituye por `c`.
        assert!(tree.collapse(PaneId::new(2)));
        assert_eq!(tree.leaves(), vec![a, c]);
        // `a` es hijo directo de la raíz: al colapsarlo queda `c` como raíz.
        assert!(tree.collapse(a));
        assert_eq!(tree.leaf_id(), Some(c));
        assert!(!tree.collapse(c), "un solo panel no se colapsa");
    }

    #[test]
    fn set_ratio_clamps_and_repairs_non_finite() {
        let (mut tree, [a, _b, _c]) = tree();
        // `a` es el primer hijo directo de la raíz: el split está en `[]`.
        let path = tree.path_to(a).expect("camino a");
        assert_eq!(path, vec![Branch::First]);
        assert!(tree.set_ratio_at(&[], 0.0));
        assert_eq!(tree.ratio_at(&[]), Some(MIN_RATIO));
        assert!(tree.set_ratio_at(&[], f32::NAN));
        assert_eq!(tree.ratio_at(&[]), Some(0.5));
    }

    #[test]
    fn sibling_is_the_other_leaf_of_the_immediate_split() {
        let (tree, [a, b, c]) = tree();
        // `a` y el subárbol `b`+`c` son hermanos; el vecino inmediato de `a`
        // es la primera hoja del otro lado del split raíz.
        assert_eq!(tree.sibling_of(a), Some(b));
        assert_eq!(tree.sibling_of(b), Some(c));
        assert_eq!(tree.sibling_of(c), Some(b));
    }

    #[test]
    fn repair_rejects_duplicate_leaves() {
        let duplicated = PaneTree::Split {
            axis: SplitAxis::Horizontal,
            ratio: 0.5,
            first: Box::new(PaneTree::leaf(PaneId::new(1))),
            second: Box::new(PaneTree::leaf(PaneId::new(1))),
        };
        let mut tree = duplicated;
        assert!(!tree.repair());
    }

    #[test]
    fn repair_fixes_ratios() {
        let mut tree = PaneTree::Split {
            axis: SplitAxis::Horizontal,
            ratio: 99.0,
            first: Box::new(PaneTree::leaf(PaneId::new(1))),
            second: Box::new(PaneTree::leaf(PaneId::new(2))),
        };
        assert!(tree.repair());
        assert_eq!(tree.ratio_at(&[]), Some(1.0 - MIN_RATIO));
    }
}
