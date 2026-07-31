// The full symbol catalogue for the equation editor's picker (ADR 0015). Each
// entry carries a Unicode glyph for the grid (cheap to render — no per-symbol
// KaTeX), the LaTeX it inserts, and search terms, so the picker behaves like an
// emoji picker: browse by category or search across everything. Only
// KaTeX-supported commands are listed, so an inserted symbol always renders.
import { EXTRA } from "./equationSymbolsExtra";

export interface EqSymbol {
  /** The glyph shown in the grid. */
  ch: string;
  /** Human name — the primary search term and the tooltip label. */
  name: string;
  /** The LaTeX command (shown in the tooltip). */
  latex: string;
  /** What to insert; defaults to `latex` + a trailing space. */
  insert?: string;
  /** Caret offset from the end of the inserted text (negative = inside braces). */
  caret?: number;
  /** Extra space-separated search terms. */
  keywords?: string;
}

export interface EqCategory {
  id: string;
  symbols: EqSymbol[];
}

/** The text actually inserted for a symbol (glyph is display-only). */
export function insertText(s: EqSymbol): string {
  return s.insert ?? `${s.latex} `;
}

/** A lowercased haystack for searching a symbol. */
export function haystack(s: EqSymbol): string {
  return `${s.name} ${s.latex} ${s.keywords ?? ""}`.toLowerCase();
}

const g = (ch: string, name: string, latex: string, keywords?: string): EqSymbol =>
  keywords !== undefined ? { ch, name, latex, keywords } : { ch, name, latex };

const BASE: EqCategory[] = [
  {
    id: "structures",
    symbols: [
      { ch: "x⁄y", name: "fraction", latex: "\\frac", insert: "\\frac{}{}", caret: -3, keywords: "over divide ratio" },
      { ch: "√", name: "square root", latex: "\\sqrt", insert: "\\sqrt{}", caret: -1, keywords: "radical" },
      { ch: "ⁿ√", name: "nth root", latex: "\\sqrt[n]", insert: "\\sqrt[]{}", caret: -3, keywords: "radical cube root" },
      { ch: "xⁿ", name: "superscript", latex: "^{}", insert: "^{}", caret: -1, keywords: "power exponent" },
      { ch: "xₙ", name: "subscript", latex: "_{}", insert: "_{}", caret: -1, keywords: "index" },
      { ch: "∑", name: "sum", latex: "\\sum", insert: "\\sum_{}^{}", caret: -3, keywords: "sigma series big" },
      { ch: "∏", name: "product", latex: "\\prod", insert: "\\prod_{}^{}", caret: -3, keywords: "big" },
      { ch: "∫", name: "integral", latex: "\\int", insert: "\\int_{}^{}", caret: -3, keywords: "big calculus" },
      { ch: "lim", name: "limit", latex: "\\lim", insert: "\\lim_{}", caret: -1, keywords: "approaches" },
      { ch: "(ⁿₖ)", name: "binomial", latex: "\\binom", insert: "\\binom{}{}", caret: -3, keywords: "choose combination" },
      { ch: "x⃗", name: "vector", latex: "\\vec", insert: "\\vec{}", caret: -1, keywords: "arrow over" },
      { ch: "x̂", name: "hat", latex: "\\hat", insert: "\\hat{}", caret: -1, keywords: "circumflex" },
      { ch: "x̄", name: "bar", latex: "\\bar", insert: "\\bar{}", caret: -1, keywords: "mean overline" },
      { ch: "ẋ", name: "dot", latex: "\\dot", insert: "\\dot{}", caret: -1, keywords: "derivative" },
      { ch: "ẍ", name: "double dot", latex: "\\ddot", insert: "\\ddot{}", caret: -1, keywords: "acceleration" },
      { ch: "x̃", name: "tilde", latex: "\\tilde", insert: "\\tilde{}", caret: -1, keywords: "approx over" },
      { ch: "x̅", name: "overline", latex: "\\overline", insert: "\\overline{}", caret: -1, keywords: "bar" },
      { ch: "x̲", name: "underline", latex: "\\underline", insert: "\\underline{}", caret: -1 },
      { ch: "⏞", name: "overbrace", latex: "\\overbrace", insert: "\\overbrace{}", caret: -1 },
      { ch: "⏟", name: "underbrace", latex: "\\underbrace", insert: "\\underbrace{}", caret: -1 },
      { ch: "[▦]", name: "matrix", latex: "\\begin{matrix}", insert: "\\begin{matrix}  \\\\  \\end{matrix}", caret: -16, keywords: "array grid table" },
      { ch: "(▦)", name: "parenthesis matrix", latex: "\\begin{pmatrix}", insert: "\\begin{pmatrix}  \\\\  \\end{pmatrix}", caret: -17, keywords: "array bracket" },
      { ch: "{▤", name: "cases", latex: "\\begin{cases}", insert: "\\begin{cases}  &  \\\\  &  \\end{cases}", caret: -20, keywords: "piecewise branch" },
    ],
  },
  {
    id: "styles",
    symbols: [
      { ch: "𝐁", name: "bold", latex: "\\mathbf", insert: "\\mathbf{}", caret: -1, keywords: "mathbf boldface" },
      { ch: "𝑖", name: "italic", latex: "\\mathit", insert: "\\mathit{}", caret: -1, keywords: "mathit" },
      { ch: "rm", name: "upright roman", latex: "\\mathrm", insert: "\\mathrm{}", caret: -1, keywords: "roman units differential dx" },
      { ch: "ℝ", name: "blackboard", latex: "\\mathbb", insert: "\\mathbb{}", caret: -1, keywords: "mathbb double struck sets reals" },
      { ch: "abc", name: "text", latex: "\\text", insert: "\\text{}", caret: -1, keywords: "plain words label" },
      { ch: "𝒜", name: "script calligraphic", latex: "\\mathcal", insert: "\\mathcal{}", caret: -1, keywords: "mathcal" },
      { ch: "𝐯", name: "bold symbol", latex: "\\boldsymbol", insert: "\\boldsymbol{}", caret: -1, keywords: "vector bold greek" },
      { ch: "𝔄", name: "fraktur", latex: "\\mathfrak", insert: "\\mathfrak{}", caret: -1, keywords: "gothic" },
      { ch: "𝖠", name: "sans serif", latex: "\\mathsf", insert: "\\mathsf{}", caret: -1, keywords: "mathsf" },
      { ch: "𝙰", name: "monospace", latex: "\\mathtt", insert: "\\mathtt{}", caret: -1, keywords: "typewriter code mathtt" },
    ],
  },
  {
    id: "greek",
    symbols: [
      g("α", "alpha", "\\alpha"), g("β", "beta", "\\beta"), g("γ", "gamma", "\\gamma"),
      g("δ", "delta", "\\delta"), g("ϵ", "epsilon", "\\epsilon"), g("ε", "varepsilon", "\\varepsilon"),
      g("ζ", "zeta", "\\zeta"), g("η", "eta", "\\eta"), g("θ", "theta", "\\theta"),
      g("ϑ", "vartheta", "\\vartheta"), g("ι", "iota", "\\iota"), g("κ", "kappa", "\\kappa"),
      g("λ", "lambda", "\\lambda"), g("μ", "mu", "\\mu"), g("ν", "nu", "\\nu"),
      g("ξ", "xi", "\\xi"), { ch: "ο", name: "omicron", latex: "o", insert: "o " }, g("π", "pi", "\\pi"),
      g("ϖ", "varpi", "\\varpi"), g("ρ", "rho", "\\rho"), g("ϱ", "varrho", "\\varrho"),
      g("σ", "sigma", "\\sigma"), g("ς", "varsigma", "\\varsigma"), g("τ", "tau", "\\tau"),
      g("υ", "upsilon", "\\upsilon"), g("ϕ", "phi", "\\phi"), g("φ", "varphi", "\\varphi"),
      g("χ", "chi", "\\chi"), g("ψ", "psi", "\\psi"), g("ω", "omega", "\\omega"),
      g("Γ", "Gamma", "\\Gamma"), g("Δ", "Delta", "\\Delta"), g("Θ", "Theta", "\\Theta"),
      g("Λ", "Lambda", "\\Lambda"), g("Ξ", "Xi", "\\Xi"), g("Π", "Pi", "\\Pi"),
      g("Σ", "Sigma", "\\Sigma"), g("Υ", "Upsilon", "\\Upsilon"), g("Φ", "Phi", "\\Phi"),
      g("Ψ", "Psi", "\\Psi"), g("Ω", "Omega", "\\Omega"),
    ],
  },
  {
    id: "operators",
    symbols: [
      g("+", "plus", "+"), g("−", "minus", "-"), g("±", "plus minus", "\\pm", "pm"),
      g("∓", "minus plus", "\\mp", "mp"), g("×", "times", "\\times", "multiply cross"),
      g("÷", "divide", "\\div", "division"), g("⋅", "centered dot", "\\cdot", "multiply"),
      g("∗", "asterisk", "\\ast"), g("⋆", "star", "\\star"), g("∘", "ring", "\\circ", "compose"),
      g("∙", "bullet", "\\bullet"), g("⊕", "circled plus", "\\oplus", "xor direct sum"),
      g("⊖", "circled minus", "\\ominus"), g("⊗", "circled times", "\\otimes", "tensor"),
      g("⊘", "circled slash", "\\oslash"), g("⊙", "circled dot", "\\odot"),
      g("⊞", "boxed plus", "\\boxplus"), g("⊠", "boxed times", "\\boxtimes"),
      g("⊡", "boxed dot", "\\boxdot"), g("†", "dagger", "\\dagger"),
      g("‡", "double dagger", "\\ddagger"), g("≀", "wreath", "\\wr"),
      g("⊓", "sqcap", "\\sqcap"), g("⊔", "sqcup", "\\sqcup"), g("⨿", "amalg", "\\amalg"),
    ],
  },
  {
    id: "relations",
    symbols: [
      g("=", "equals", "="), g("≠", "not equal", "\\ne", "neq"), g("<", "less than", "<"),
      g(">", "greater than", ">"), g("≤", "less or equal", "\\le", "leq"),
      g("≥", "greater or equal", "\\ge", "geq"), g("≪", "much less", "\\ll"),
      g("≫", "much greater", "\\gg"), g("≈", "approximately", "\\approx", "approx"),
      g("≡", "identical", "\\equiv", "equivalent congruent"), g("≅", "congruent", "\\cong"),
      g("∼", "similar", "\\sim"), g("≃", "similar equal", "\\simeq"),
      g("∝", "proportional", "\\propto"), g("≐", "dot equal", "\\doteq"),
      g("≺", "precedes", "\\prec"), g("≻", "succeeds", "\\succ"),
      g("⪯", "precedes equal", "\\preceq"), g("⪰", "succeeds equal", "\\succeq"),
      g("≍", "asymptotic", "\\asymp"), g("⋈", "bowtie", "\\bowtie"),
      g("⊨", "models", "\\models"), g("⊢", "proves", "\\vdash"), g("⊣", "dashv", "\\dashv"),
      g("⊥", "perpendicular", "\\perp", "orthogonal"), g("∥", "parallel", "\\parallel"),
      g("∣", "divides", "\\mid"), g("≥", "geqslant", "\\geqslant"), g("≤", "leqslant", "\\leqslant"),
    ],
  },
  {
    id: "sets",
    symbols: [
      g("∈", "element of", "\\in", "member"), g("∉", "not element of", "\\notin"),
      g("∋", "contains", "\\ni"), g("⊂", "subset", "\\subset"), g("⊃", "superset", "\\supset"),
      g("⊆", "subset equal", "\\subseteq"), g("⊇", "superset equal", "\\supseteq"),
      g("⊊", "proper subset", "\\subsetneq"), g("⊋", "proper superset", "\\supsetneq"),
      g("∪", "union", "\\cup"), g("∩", "intersection", "\\cap"),
      g("∖", "set minus", "\\setminus", "difference"), g("∅", "empty set", "\\varnothing", "emptyset null"),
      g("∁", "complement", "\\complement"), g("ℕ", "natural numbers", "\\mathbb{N}", "blackboard"),
      g("ℤ", "integers", "\\mathbb{Z}", "blackboard"), g("ℚ", "rationals", "\\mathbb{Q}", "blackboard"),
      g("ℝ", "reals", "\\mathbb{R}", "blackboard real"), g("ℂ", "complex", "\\mathbb{C}", "blackboard"),
      g("∀", "for all", "\\forall", "universal"), g("∃", "there exists", "\\exists"),
      g("∄", "not exists", "\\nexists"), g("¬", "not", "\\neg", "negation"),
      g("∧", "and", "\\land", "wedge conjunction"), g("∨", "or", "\\lor", "vee disjunction"),
      g("⟹", "implies", "\\implies"), g("⟺", "if and only if", "\\iff", "equivalent"),
      g("∴", "therefore", "\\therefore"), g("∵", "because", "\\because"),
      g("⊤", "top", "\\top", "true"), g("⊥", "bottom", "\\bot", "false"),
    ],
  },
  {
    id: "arrows",
    symbols: [
      g("→", "right arrow", "\\to", "rightarrow maps"), g("←", "left arrow", "\\gets", "leftarrow"),
      g("↔", "left right arrow", "\\leftrightarrow"), g("⇒", "implies arrow", "\\Rightarrow"),
      g("⇐", "left double arrow", "\\Leftarrow"), g("⇔", "iff arrow", "\\Leftrightarrow"),
      g("↦", "maps to", "\\mapsto"), g("⟶", "long right arrow", "\\longrightarrow"),
      g("⟵", "long left arrow", "\\longleftarrow"), g("⟷", "long left right", "\\longleftrightarrow"),
      g("⟹", "long implies", "\\Longrightarrow"), g("↑", "up arrow", "\\uparrow"),
      g("↓", "down arrow", "\\downarrow"), g("↕", "up down arrow", "\\updownarrow"),
      g("⇑", "up double arrow", "\\Uparrow"), g("⇓", "down double arrow", "\\Downarrow"),
      g("↗", "north east", "\\nearrow"), g("↘", "south east", "\\searrow"),
      g("↙", "south west", "\\swarrow"), g("↖", "north west", "\\nwarrow"),
      g("↪", "hook right", "\\hookrightarrow"), g("↩", "hook left", "\\hookleftarrow"),
      g("⇀", "right harpoon", "\\rightharpoonup"), g("↼", "left harpoon", "\\leftharpoonup"),
      g("⇌", "harpoons", "\\rightleftharpoons", "equilibrium"), g("↠", "two head right", "\\twoheadrightarrow"),
      g("⇆", "left right arrows", "\\leftrightarrows"), g("⟼", "long maps to", "\\longmapsto"),
    ],
  },
  {
    id: "bigops",
    symbols: [
      { ch: "∑", name: "sum", latex: "\\sum", insert: "\\sum_{}^{}", caret: -3, keywords: "sigma series" },
      { ch: "∏", name: "product", latex: "\\prod", insert: "\\prod_{}^{}", caret: -3 },
      { ch: "∐", name: "coproduct", latex: "\\coprod", insert: "\\coprod_{}^{}", caret: -3 },
      { ch: "∫", name: "integral", latex: "\\int", insert: "\\int_{}^{}", caret: -3 },
      { ch: "∬", name: "double integral", latex: "\\iint", insert: "\\iint_{}^{}", caret: -3 },
      { ch: "∭", name: "triple integral", latex: "\\iiint", insert: "\\iiint_{}^{}", caret: -3 },
      { ch: "∮", name: "contour integral", latex: "\\oint", insert: "\\oint_{}^{}", caret: -3, keywords: "loop" },
      { ch: "⋃", name: "big union", latex: "\\bigcup", insert: "\\bigcup_{}^{}", caret: -3 },
      { ch: "⋂", name: "big intersection", latex: "\\bigcap", insert: "\\bigcap_{}^{}", caret: -3 },
      { ch: "⨆", name: "big sqcup", latex: "\\bigsqcup", insert: "\\bigsqcup_{}^{}", caret: -3 },
      { ch: "⋁", name: "big vee", latex: "\\bigvee", insert: "\\bigvee_{}^{}", caret: -3, keywords: "or" },
      { ch: "⋀", name: "big wedge", latex: "\\bigwedge", insert: "\\bigwedge_{}^{}", caret: -3, keywords: "and" },
      { ch: "⨁", name: "big oplus", latex: "\\bigoplus", insert: "\\bigoplus_{}^{}", caret: -3 },
      { ch: "⨂", name: "big otimes", latex: "\\bigotimes", insert: "\\bigotimes_{}^{}", caret: -3 },
      { ch: "⨀", name: "big odot", latex: "\\bigodot", insert: "\\bigodot_{}^{}", caret: -3 },
      g("lim", "limit", "\\lim", "approaches"), g("sup", "supremum", "\\sup"),
      g("inf", "infimum", "\\inf"), g("max", "maximum", "\\max"), g("min", "minimum", "\\min"),
      g("gcd", "gcd", "\\gcd"), g("det", "determinant", "\\det"), g("deg", "degree", "\\deg"),
    ],
  },
  {
    id: "calculus",
    symbols: [
      g("∂", "partial", "\\partial", "derivative"), g("∇", "nabla", "\\nabla", "del gradient"),
      g("∞", "infinity", "\\infty"), g("′", "prime", "\\prime", "derivative"),
      g("″", "double prime", "\\prime\\prime"), g("d", "differential", "\\mathrm{d}", "dx"),
      g("ℓ", "script l", "\\ell"), g("ℏ", "h bar", "\\hbar", "planck"),
      g("ℵ", "aleph", "\\aleph"), g("ℜ", "real part", "\\Re"), g("ℑ", "imaginary part", "\\Im"),
      g("℘", "weierstrass", "\\wp"), g("∠", "angle", "\\angle"),
      g("∡", "measured angle", "\\measuredangle"), g("∢", "spherical angle", "\\sphericalangle"),
      g("△", "triangle", "\\triangle"), g("□", "square", "\\square"),
      g("◇", "diamond", "\\diamond"), g("∎", "end of proof", "\\blacksquare", "qed tombstone"),
      g("∝", "proportional", "\\propto"), g("∅", "empty", "\\emptyset"),
    ],
  },
  {
    id: "delimiters",
    symbols: [
      g("⟨", "left angle", "\\langle"), g("⟩", "right angle", "\\rangle"),
      g("⌈", "left ceiling", "\\lceil"), g("⌉", "right ceiling", "\\rceil"),
      g("⌊", "left floor", "\\lfloor"), g("⌋", "right floor", "\\rfloor"),
      g("{", "left brace", "\\{", "curly"), g("}", "right brace", "\\}", "curly"),
      g("|", "vertical bar", "\\vert", "abs modulus"), g("‖", "double bar", "\\Vert", "norm"),
      { ch: "( )", name: "auto parentheses", latex: "\\left(\\right)", insert: "\\left(\\right)", caret: -7, keywords: "sizing bracket" },
      { ch: "[ ]", name: "auto brackets", latex: "\\left[\\right]", insert: "\\left[\\right]", caret: -7 },
      { ch: "| |", name: "auto absolute", latex: "\\left|\\right|", insert: "\\left|\\right|", caret: -7, keywords: "modulus" },
    ],
  },
  {
    id: "misc",
    symbols: [
      g("⋯", "center dots", "\\cdots"), g("…", "low dots", "\\ldots", "ellipsis"),
      g("⋮", "vertical dots", "\\vdots"), g("⋱", "diagonal dots", "\\ddots"),
      { ch: "°", name: "degree", latex: "^\\circ", insert: "{}^\\circ ", keywords: "temperature" }, g("%", "percent", "\\%"),
      g("$", "dollar", "\\$"), g("#", "hash", "\\#"), g("&", "ampersand", "\\&"),
      g("✓", "check mark", "\\checkmark", "tick"), g("✠", "maltese", "\\maltese"),
      g("♠", "spade", "\\spadesuit"), g("♡", "heart", "\\heartsuit"),
      g("♢", "diamond suit", "\\diamondsuit"), g("♣", "club", "\\clubsuit"),
      g("♭", "flat", "\\flat", "music"), g("♮", "natural", "\\natural", "music"),
      g("♯", "sharp", "\\sharp", "music"), g("§", "section", "\\S"), g("¶", "paragraph", "\\P"),
      g("©", "copyright", "\\copyright"), g("∠", "angle", "\\angle"),
    ],
  },
];

/** The full catalogue: the hand-curated symbols first (nice names + keywords),
 * then every remaining KaTeX symbol appended to its category (auto-generated in
 * `equationSymbolsExtra.ts`), so the picker lists the complete KaTeX set. */
export const EQ_CATEGORIES: EqCategory[] = BASE.map((c) => ({
  ...c,
  symbols: [...c.symbols, ...(EXTRA[c.id] ?? [])],
}));
