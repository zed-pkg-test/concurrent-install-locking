//! Independent acceptance harness for the production `zed-lock` package.
//!
//! The executable behavior lives in integration tests so each case runs in a
//! separate test process and can spawn additional contenders safely.
