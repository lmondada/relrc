use std::collections::BTreeSet;

use mpi::traits::Communicator;
use relrc::{MPIRecvRelRc, MPISendRelRc, RelRc};

fn count_ancestors(relrc: &RelRc<usize, usize>) -> usize {
    relrc.detach(&BTreeSet::new()).n_ancestors()
}

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let size = world.size();
    let rank = world.rank();

    if size != 2 {
        panic!("Size of MPI_COMM_WORLD must be 2, but is {}!", size);
    }

    // The first process has a set of 4 RelRc objects, the second has 2 of them.
    let mut all_relrcs = match rank {
        0 => {
            // Create the first set of RelRc objects forming a diamond pattern
            let root1 = RelRc::new(0);
            let left_child1 = RelRc::with_parents(1, vec![(root1.clone(), 1)]);
            let right_child1 = RelRc::with_parents(2, vec![(root1.clone(), 2)]);
            let grandchild1 = RelRc::with_parents(
                3,
                vec![(left_child1.clone(), 11), (right_child1.clone(), 12)],
            );

            vec![root1, left_child1, right_child1, grandchild1]
        }
        1 => {
            // Create the second set of RelRc objects
            let root2 = RelRc::new(0);
            let left_child2 = RelRc::with_parents(1, vec![(root2.clone(), 1)]);

            vec![root2, left_child2]
        }
        _ => unreachable!(),
    };

    println!(
        "[begin] rank {rank}: total number of objects: {}",
        count_ancestors(all_relrcs.last().unwrap())
    );

    // Sending the grandchild of process 0 to process 1 should create 2 new
    // RelRc objects in process 1.
    match rank {
        0 => {
            world.process_at_rank(1).send_relrc(&all_relrcs[3]);
            println!("rank {rank}: sent grandchild");
        }
        1 => {
            let left_child2 = all_relrcs[1].clone();
            let received = world.process_at_rank(0).recv_relrc(all_relrcs.clone());
            println!("rank {rank}: received grandchild");

            assert_eq!(count_ancestors(&received), 4);
            assert_eq!(received.all_parents().len(), 2);
            assert!(received
                .all_parents()
                .any(|c| RelRc::ptr_eq(c, &left_child2)));

            all_relrcs.push(received);
        }
        _ => unreachable!(),
    }

    println!(
        "[end] rank {rank}: total number of objects: {}",
        count_ancestors(all_relrcs.last().unwrap())
    );

    println!("done at rank {}", rank);
}
