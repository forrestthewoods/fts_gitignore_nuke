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

#[test]
fn simple_stack() {
    let root = ImmutableStack::new();
    let tip = root.push(1).push(2).push(3);
    assert_eq!(tip.iter().cloned().collect::<Vec<_>>(), vec![3,2,1]);
}

#[test]
fn split_stack() {
    let tip = ImmutableStack::new()
        .push(1)
        .push(2)
        .push(3);

    let a = tip
        .push(4)
        .push(5);
    let b = tip
        .push(99)
        .push(100);

    assert_eq!(tip.iter().cloned().collect::<Vec<_>>(), vec![3,2,1]);
    assert_eq!(a.iter().cloned().collect::<Vec<_>>(), vec![5,4,3,2,1]);
    assert_eq!(b.iter().cloned().collect::<Vec<_>>(), vec![100,99,3,2,1]);
}

#[test]
fn parallel_stack() {
    let tip = ImmutableStack::new()
        .push(1)
        .push(2)
        .push(3);

    for i in 0..5 {
        let tip_copy = tip.clone();
        std::thread::spawn(move ||{
            let thread_tip = tip_copy
                .push(i)
                .push(i*2);
            assert_eq!(thread_tip.iter().cloned().collect::<Vec<_>>(), vec![i*2,i,3,2,1]);
        });
    }
}