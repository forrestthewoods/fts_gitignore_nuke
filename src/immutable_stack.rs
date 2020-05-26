use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ImmutableStack<T> {
    node: Link<T>,
}

type Link<T> = Option<Arc<Node<T>>>;

pub(crate) struct Node<T> {
    elem: T,
    next: Link<T>,
}

impl<T> ImmutableStack<T> {
    pub fn new() -> Self {
        ImmutableStack { node: None }
    }

    pub fn push(&self, elem: T) -> ImmutableStack<T> {
        ImmutableStack { 
            node: Some(
                Arc::new(
                    Node {
                        elem,
                        next: self.node.clone(),
                    }
                )
            )
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter { next: self.node.as_ref().map(|node| &**node) }
    }
}

impl<T> Drop for ImmutableStack<T> {
    fn drop(&mut self) {
        let mut iter = self.node.take();
        while let Some(node) = iter {
            if let Ok(mut node) = Arc::try_unwrap(node) {
                iter = node.next.take();
            } else {
                break;
            }
        }
    }
}

pub(crate) struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.map(|node| {
            self.next = node.next.as_ref().map(|node| &**node);
            &node.elem
        })
    }
}