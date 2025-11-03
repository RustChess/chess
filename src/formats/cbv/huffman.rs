use super::*;

/// Huffman tree
#[derive(Default)]
pub struct Tree {
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
    pub value: Option<u8>,
}

impl From<u8> for Tree {
    fn from(value: u8) -> Self {
        Self { left: None, right: None, value: Some(value) }
    }
}

impl Tree {
    /// Descend into the tree in the specified direction (right or left).
    /// When a leaf is reached, push the value to result and reset the visitor.
    // fn visit<'a>(mut current: &mut &'a Tree, root: &'a Tree, result: &mut Vec<u8>, right: bool) {
    fn visit<'a>(
        &'a self,
        current: &mut &'a Tree,
        result: &mut Vec<u8>,
        right: bool,
    ) -> Option<()> {
        let direction = if right { current.right.as_ref()? } else { current.left.as_ref()? };
        if let Some(byte) = direction.value {
            result.push(byte);
            *current = self;
        } else {
            *current = direction;
        }
        Some(())
    }

    #[must_use]
    // None denotes failure
    pub fn unpack(&self, bits: Bits<'_>, decompressed: usize) -> Option<Vec<u8>> {
        let mut current = self;
        let mut result = Vec::new();

        for direction in bits {
            if result.len() == decompressed {
                break;
            }

            self.visit(&mut current, &mut result, direction)?;
        }

        Some(result)
    }

    pub fn get_mut_or_insert(&mut self, right: bool) -> &mut Self {
        if right { self.right.get_or_insert_default() } else { self.left.get_or_insert_default() }
    }

    pub fn load(values: [(usize, u16); 256]) -> Self {
        let mut tree = Tree::default();
        for (i, (len, bits)) in values.into_iter().enumerate() {
            if len > 0 {
                let mut node = &mut tree;
                let bits = (bits << (16 - len)).to_be_bytes();

                for direction in Bits::from(bits.as_slice()).take(len) {
                    node = node.get_mut_or_insert(direction);
                }

                node.value = Some(i as u8);
            }
        }
        tree
    }
}
