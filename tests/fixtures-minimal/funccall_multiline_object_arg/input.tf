locals {
  combined = merge(
    base,
    {
      "key1" = "value1"
      "key2" = "value2"
    }
  )
}
