locals {
  content = templatefile("${path.module}/script.ps1", ({
    app_db_name              = var.app != null ? var.app.database_name : "AppDB"
    app_enabled              = var.app != null
    app_service_account_name = var.app != null ? var.app.service_account : ""
    app_url                  = var.app != null ? var.app.url : "http://localhost"
    dd_site                  = var.dd != null ? var.dd.site : ""
    dd_tags                  = var.dd != null ? var.dd.tags : {}
  }))
}
