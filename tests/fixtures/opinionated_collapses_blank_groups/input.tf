resource "aws_cognito_identity_provider" "google" {
  user_pool_id = aws_cognito_user_pool.serverless_auth.id


  provider_name = "Google"
  provider_type = "Google"

  provider_details = {
    attributes_url                = "https://people.googleapis.com/v1/people/me?personFields="
    attributes_url_add_attributes = "true"

    client_id                     = local.secrets["cognito_google_client_id"]
    client_secret                 = local.secrets["cognito_google_client_secret"]
  }

  attribute_mapping = {
    email    = "email"
    username = "sub"
  }
}
