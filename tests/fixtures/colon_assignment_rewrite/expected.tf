locals {
  m = {
    "attribute.inline_very_long" = "assertion.inline_very_long_name"
    (local.names["short"])       = "assertion.short"
    (local.names["very_long"])   = "assertion.very_long"
  }
}
