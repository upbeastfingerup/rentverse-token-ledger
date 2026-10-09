const express = require("express");
const axios = require("axios");

const router = express.Router();
const LEDGER_URL = process.env.LEDGER_API_URL || "http://127.0.0.1:8080";

// Forward a request to the Rust ledger service and relay status + body as-is,
// including its structured error payloads (4xx).
const forward = (method, path) => async (req, res) => {
  try {
    const target = typeof path === "function" ? path(req) : path;
    const { status, data } = await axios({
      method,
      url: `${LEDGER_URL}${target}`,
      data: method === "get" ? undefined : req.body,
      timeout: 5000,
    });
    res.status(status).json(data);
  } catch (err) {
    if (err.response) return res.status(err.response.status).json(err.response.data);
    res.status(502).json({ error: { code: "ledger_unavailable", message: "Ledger service is unreachable" } });
  }
};

router.get("/health", forward("get", "/health"));
router.post("/mint", forward("post", "/mint"));
router.post("/transfer", forward("post", "/transfer"));
router.get("/balances/:account", forward("get", (req) => `/balances/${encodeURIComponent(req.params.account)}`));
router.get("/supply", forward("get", "/supply"));
router.get("/events", forward("get", "/events"));

module.exports = router;
