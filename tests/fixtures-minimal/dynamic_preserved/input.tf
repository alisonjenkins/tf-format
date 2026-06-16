resource "example" "this" {
  dynamic "item" {
    content {
      value = item.value
    }

    for_each = toset(["a", "b"])
  }
}
