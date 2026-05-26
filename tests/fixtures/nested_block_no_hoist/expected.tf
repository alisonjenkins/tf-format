resource "google_cloud_run_v2_job" "this" {
  location = "us-central1"
  name     = "example"

  containers {
    env {
      name = "SUPERUSER_PASSWORD"

      value_source {
        secret_key_ref {
          secret  = var.superuser_password_secret
          version = var.superuser_password_secret_version
        }
      }
    }
  }
}
