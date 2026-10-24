# Margolus block cellular automata
Source: Toffoli and Margolus, "Cellular Automata Machines", MIT Press 1987, ch. II.12 "The Margolus neighborhood", pp. 119-138 (book PDF at https://people.csail.mit.edu/nhm/cam-book.pdf was too large to fetch; claims checked via the Wikipedia "Block cellular automaton" summary, which cites the book).
Adopt: a 2x2x2 partition shifted by one cell on alternate steps; block-local rules that conserve the cell count per block, so global conservation follows; block rules that are permutations, so reversibility follows.
Reject: the Critters and Tron rules as game rules; any expectation that block CAs add computational power (the book says they do not beyond a standard CA).
Why: Our 2x2x2 Margolus kernel gets conservation and reversibility by construction, keeping integer-only state deterministic and letting parity-split dispatch run without races on the iGPU.
