// Spotlight's calculator: + - * / ^, parentheses, decimals and unary signs.
// A small recursive-descent parser; the page's CSP forbids eval (rightly).

const SYMBOLS = { "×": "*", "÷": "/", "−": "-" };

function tokenize(text) {
  const tokens = [];
  const s = text.replace(/\*\*/g, "^");
  let i = 0;
  while (i < s.length) {
    const c = SYMBOLS[s[i]] ?? s[i];
    if (/\s/.test(c)) {
      i++;
    } else if ("+-*/^()".includes(c)) {
      tokens.push(c);
      i++;
    } else {
      const number = /^(\d+\.?\d*|\.\d+)/.exec(s.slice(i));
      if (!number) return null;
      tokens.push(Number(number[0]));
      i += number[0].length;
    }
  }
  return tokens;
}

function parse(tokens) {
  let pos = 0;
  let binary = 0;
  const peek = () => tokens[pos];
  const take = (...ops) => (ops.includes(peek()) ? tokens[pos++] : null);

  // expr = term { (+|-) term }
  const expr = () => {
    let value = term();
    for (let op; (op = take("+", "-")); binary++) value = op === "+" ? value + term() : value - term();
    return value;
  };
  // term = unary { (*|/) unary }
  const term = () => {
    let value = unary();
    for (let op; (op = take("*", "/")); binary++) value = op === "*" ? value * unary() : value / unary();
    return value;
  };
  // unary = (+|-) unary | power, so -2^2 is -(2^2)
  const unary = () => {
    const sign = take("+", "-");
    if (sign) return sign === "-" ? -unary() : unary();
    return power();
  };
  // power = primary [^ unary], right-associative
  const power = () => {
    const base = primary();
    if (!take("^")) return base;
    binary++;
    return base ** unary();
  };
  const primary = () => {
    const token = tokens[pos++];
    if (typeof token === "number") return token;
    if (token === "(") {
      const value = expr();
      if (!take(")")) throw new SyntaxError("missing )");
      return value;
    }
    throw new SyntaxError("expected a number");
  };

  const value = expr();
  if (pos !== tokens.length) throw new SyntaxError("unexpected input");
  return { value, binary };
}

/** 12 significant digits hide floating-point noise (0.1 + 0.2 shows 0.3). */
export function formatNumber(value) {
  return String(Number(value.toPrecision(12)));
}

const DISPLAY = { "*": "×", "/": "÷" };

/** "3*6+6-45" -> "3 × 6 + 6 - 45": spaced binary operators; signs stay on their numbers. */
function formatExpression(tokens) {
  return tokens
    .map((t, i) => {
      const prev = tokens[i - 1];
      const sign = (t === "+" || t === "-") && (i === 0 || (typeof prev !== "number" && prev !== ")"));
      if (typeof t === "number" || t === "(" || t === ")" || t === "^" || sign) return String(t);
      return ` ${DISPLAY[t] ?? t} `;
    })
    .join("");
}

/**
 * `{ expression, answer }` for `text` if it is arithmetic with at least one
 * operator ("2+2", "3*6+6-45", "(1+2)^3"); otherwise null. A lone number
 * ("42", "-5") or anything incomplete or undefined (1/0) is null.
 */
export function calculate(text) {
  const tokens = tokenize(text.trim());
  if (!tokens?.length) return null;
  try {
    const { value, binary } = parse(tokens);
    if (binary === 0 || !Number.isFinite(value)) return null;
    return { expression: formatExpression(tokens), answer: formatNumber(value) };
  } catch {
    return null;
  }
}
