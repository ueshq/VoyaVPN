import ts from "typescript";

const VISIBLE_JSX_ATTRIBUTES = new Set([
  "alt",
  "aria-description",
  "aria-label",
  "description",
  "emptyText",
  "label",
  "placeholder",
  "title",
  "tooltip",
]);

const VISIBLE_OBJECT_PROPERTIES = new Set([
  "ariaLabel",
  "description",
  "emptyText",
  "label",
  "placeholder",
  "title",
  "tooltip",
]);

const UI_HELPER_NAME = /(?:Description|Label|Message|Placeholder|Status|Summary|Title|Tooltip)$/u;

const TECHNICAL_TEXT_ALLOWLIST = new Set([
  "AES-128-GCM",
  "AES-256-GCM",
  "AnyTLS",
  "BBR",
  "ChaCha20-Poly1305",
  "Custom",
  "Ctrl",
  "DNS",
  "HTTP",
  "HTTPS",
  "Hysteria 2",
  "Hysteria2",
  "HTTP Upgrade",
  "F",
  "IP",
  "IPv6",
  "JSON",
  "KCP",
  "KB/s",
  "MB/s",
  "Mbps",
  "NaiveProxy",
  "Naive",
  "None",
  "Process TUN",
  "Policy Group",
  "Proxy Chain",
  "QR",
  "QUIC",
  "REALITY",
  "SOCKS",
  "Shadowsocks",
  "TCP",
  "TCP / Raw",
  "TLS",
  "TUN",
  "TUIC",
  "Trojan",
  "UDP",
  "URL",
  "VLESS",
  "VMess",
  "VoyaVPN",
  "WebSocket",
  "WireGuard",
  "XHTTP",
  "macOS PacketTunnel",
  "gRPC",
  "sing-box",
  "auto",
  "bbr",
  "none",
  "xtls-rprx-vision",
  "^US|Japan",
]);

const TECHNICAL_TEXT_PATTERNS = [
  /^(?:https?|socks5):\/\//u,
  /^(?:aes|chacha20|xchacha20|2022-blake3)-[a-z0-9-]+$/iu,
  /^\/[A-Za-z0-9._~!$&'()*+,;=:@%/-]*$/u,
  // Upper-case *identifiers*, not upper-case words: the token must carry a
  // digit or separator (SOCKS5, HTTP/2, AES-256, X25519). The previous
  // `^[A-Z][A-Z0-9_.+/-]{1,15}$` accepted any short shout-case word, so
  // `OK`, `SAVE`, `CANCEL`, `ERROR` and `RETRY` all passed as "technical".
  /^[A-Z][A-Z0-9]*[0-9_.+/-][A-Z0-9_.+/-]*$/u,
];

export function inspectI18nSource({ path, source, knownKeys = new Set() }) {
  const sourceFile = ts.createSourceFile(
    path,
    source,
    ts.ScriptTarget.Latest,
    true,
    path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const invalidKeys = [];
  const dynamicKeys = [];
  const hardcodedText = [];

  function recordText(node, detail = textFromNode(node)) {
    const text = normalizeText(detail);
    if (isUserVisibleText(text)) {
      hardcodedText.push(sourceLocation(sourceFile, node, text));
    }
  }

  function inspectVisibleExpression(node) {
    if (ts.isParenthesizedExpression(node)) {
      inspectVisibleExpression(node.expression);
    } else if (isTextLiteral(node)) {
      recordText(node);
    } else if (ts.isConditionalExpression(node)) {
      inspectVisibleExpression(node.whenTrue);
      inspectVisibleExpression(node.whenFalse);
    } else if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.PlusToken) {
      inspectVisibleExpression(node.left);
      inspectVisibleExpression(node.right);
    } else if (isRenderedFallback(node)) {
      // `{value || "Unknown"}` and `{value ?? "Unknown"}` render either side, so
      // both operands are user-visible; `{loading && "Loading"}` renders only
      // the right one. Without these the fallback string is an untranslated
      // literal that still reaches the screen in every locale.
      if (node.operatorToken.kind !== ts.SyntaxKind.AmpersandAmpersandToken) {
        inspectVisibleExpression(node.left);
      }
      inspectVisibleExpression(node.right);
    } else if (ts.isTemplateExpression(node)) {
      recordText(node.head, node.head.text);
      for (const span of node.templateSpans) {
        recordText(span.literal, span.literal.text);
        inspectVisibleExpression(span.expression);
      }
    }
  }

  function visit(node) {
    if (ts.isCallExpression(node) && isTranslationCall(node.expression) && node.arguments.length > 0) {
      const [key] = node.arguments;
      if (isTextLiteral(key)) {
        validateStaticKey(key);
      } else if (ts.isConditionalExpression(key)) {
        validateStaticKey(key.whenTrue);
        validateStaticKey(key.whenFalse);
      } else if (ts.isTemplateExpression(key) || ts.isBinaryExpression(key)) {
        dynamicKeys.push(sourceLocation(sourceFile, key, key.getText(sourceFile)));
      }
    }

    if (ts.isJsxText(node)) {
      recordText(node, node.text);
    } else if (ts.isJsxAttribute(node) && VISIBLE_JSX_ATTRIBUTES.has(node.name.text)) {
      if (node.initializer && ts.isStringLiteral(node.initializer)) {
        recordText(node.initializer);
      } else if (node.initializer && ts.isJsxExpression(node.initializer) && node.initializer.expression) {
        inspectVisibleExpression(node.initializer.expression);
      }
    } else if (
      ts.isJsxExpression(node)
      && node.expression
      && !ts.isJsxAttribute(node.parent)
    ) {
      inspectVisibleExpression(node.expression);
    } else if (ts.isPropertyAssignment(node) && propertyName(node.name)) {
      if (VISIBLE_OBJECT_PROPERTIES.has(propertyName(node.name))) {
        inspectVisibleExpression(node.initializer);
      }
    } else if (ts.isReturnStatement(node) && node.expression && isUiHelperReturn(node)) {
      inspectVisibleExpression(node.expression);
    }

    ts.forEachChild(node, visit);
  }

  function validateStaticKey(node) {
    if (!isTextLiteral(node)) {
      dynamicKeys.push(sourceLocation(sourceFile, node, node.getText(sourceFile)));
    } else if (!knownKeys.has(node.text)) {
      invalidKeys.push(sourceLocation(sourceFile, node, node.text));
    }
  }

  visit(sourceFile);
  return { dynamicKeys, hardcodedText, invalidKeys };
}

/**
 * Hardcoded strings that predate the `||` / `??` / `&&` fallback rule and live
 * in files this tooling batch may not edit. Matched on repository-relative path
 * plus the exact normalized text, so an entry cannot suppress anything else.
 * A stale entry is reported by `i18n.mjs`, never failed, so the owning feature
 * can delete it whenever the locale key lands.
 *
 * Empty on purpose: keep it that way. The last entry covered `keyCodeLabel`'s
 * `Key ${keyCode}` fallback in general-tab.tsx, which now resolves
 * `options.keyName.*` like every other named keycap.
 */
export const KNOWN_HARDCODED_TEXT = [];

export function isKnownHardcodedText(relativePath, text, allowlist = KNOWN_HARDCODED_TEXT) {
  return allowlist.some((entry) => entry.path === relativePath && entry.text === text);
}

export function isUserVisibleText(value) {
  const text = normalizeText(value);
  // Detect letters in any script so all hardcoded UI text uses the same rule.
  if (text.length === 0 || !/\p{L}/u.test(text)) {
    return false;
  }
  if (TECHNICAL_TEXT_ALLOWLIST.has(text)) {
    return false;
  }
  return !TECHNICAL_TEXT_PATTERNS.some((pattern) => pattern.test(text));
}

function isUiHelperReturn(node) {
  let current = node.parent;
  while (current) {
    if (ts.isFunctionDeclaration(current)) {
      return Boolean(current.name && UI_HELPER_NAME.test(current.name.text));
    }
    if (ts.isArrowFunction(current) || ts.isFunctionExpression(current)) {
      const parent = current.parent;
      return ts.isVariableDeclaration(parent)
        && ts.isIdentifier(parent.name)
        && UI_HELPER_NAME.test(parent.name.text);
    }
    current = current.parent;
  }
  return false;
}

const RENDERED_FALLBACK_OPERATORS = new Set([
  ts.SyntaxKind.BarBarToken,
  ts.SyntaxKind.QuestionQuestionToken,
  ts.SyntaxKind.AmpersandAmpersandToken,
]);

function isRenderedFallback(node) {
  return ts.isBinaryExpression(node) && RENDERED_FALLBACK_OPERATORS.has(node.operatorToken.kind);
}

function isTextLiteral(node) {
  return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node);
}

function isTranslationCall(expression) {
  return (ts.isIdentifier(expression) && expression.text === "t")
    || (ts.isPropertyAccessExpression(expression) && expression.name.text === "t");
}

function normalizeText(value) {
  return String(value).replace(/^["'`]|["'`]$/gu, "").replace(/\s+/gu, " ").trim();
}

function propertyName(node) {
  return ts.isIdentifier(node) || ts.isStringLiteral(node) ? node.text : null;
}

function sourceLocation(sourceFile, node, detail) {
  const { line } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return { detail, line: line + 1 };
}

function textFromNode(node) {
  return "text" in node ? node.text : node.getText();
}
