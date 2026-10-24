# Utility AI (Dave Mark; Rain World creature AI)
Source: Dave Mark, "Behavioral Mathematics for Game AI", Course Technology, 2009 (ISBN 978-1584506843); Mark and Kevin Dill, GDC AI Summit "Improving AI Decision Modeling Through Utility Theory", 2010 (cited via Wikipedia "Utility system", unverified); Joar Jakobsson, Rain World creature talk (GDC, year and slides unverified).
Adopt: Response curves mapping normalized inputs to scores, with the highest-scoring action chosen; survival-driven needs (food, shelter, time of day) as the drivers for creature behaviour.
Reject: Per-frame float scoring of every creature; unbounded needs-driven loops that can stall a tick; Rain World's physics-puppet animation.
Why: Curves can be lookup tables with integer scores, and budgeted per-creature evaluation fits the factory and adventure tick, but float curves break determinism.
