# Noita: GDC 2019 (Petri Purho)
Source: Petri Purho, "Exploring the Tech and Design of Noita", GDC 2019, https://www.gdcvault.com/play/1025695 (talk exists; slide details only via secondary sources: Macuyiko blog and 80.lv article, so the 64x64 chunk, 4-pass checkerboard and 32-cell margin figures are unverified).
Adopt: per-chunk dirty rects so idle regions cost nothing; a multi-pass checkerboard ordering so chunks update in parallel without two workers touching one cell; a separate pass for pixels displaced by bodies.
Reject: float rigid bodies inside the cellular state, and the double-buffer approach (secondary source says Noita uses no second buffer).
Why: Pass ordering maps onto our Margolus 2x2x2 parity split and keeps updates integer and order-independent, whereas float bodies would break determinism.
