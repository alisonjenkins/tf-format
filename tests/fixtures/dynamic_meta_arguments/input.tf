resource "example" "this" {
  dynamic "item" {
    content {
      value = item.value
    }

    for_each = toset([
      "a",
      "b",
    ])
  }

  dynamic "setting" {
    content {
      namespace = setting.value.namespace
      name      = setting.key
    }

    labels   = ["outer", "inner"]
    iterator = setting
    for_each = var.settings
  }

  dynamic "outer" {
    for_each = var.groups
    content {
      dynamic "inner" {
        content {
          v = inner.value
        }
        for_each = outer.value.items
      }
    }
  }
}
