# CLAUDE PLACEHOLDER ARTICLE: Notes on Rust

Learning Rust felt like learning to program all over again.

The borrow checker is infamous for rejecting code that "obviously" works. But after a while, you start to see what it sees. Those "obviously correct" programs often had subtle bugs waiting to happen.

## The Mental Model Shift

- Stop thinking about objects with identity
- Start thinking about values with ownership
- Data flows through your program like water

Once it clicks, you find yourself writing better code in other languages too. You notice when you're holding references too long, when ownership is unclear, when data races could occur.

## A Mathematical Aside

Ownership can be thought of formally. If we let $O(v)$ represent the owner of value $v$, then:

$$\forall v : |O(v)| = 1$$

That is, every value has exactly one owner at any given time. The borrow checker enforces this invariant at compile time, which is why Rust can guarantee memory safety without a garbage collector.

The complexity of the borrow checker is roughly $O(n \cdot m)$ where $n$ is the number of variables and $m$ is the number of lifetimes.

## The Bigger Lesson

Rust didn't teach me how to write Rust. It taught me how to think about memory, concurrency, and program structure in a deeper way.

*This website is written in Rust, compiled to WebAssembly.*
