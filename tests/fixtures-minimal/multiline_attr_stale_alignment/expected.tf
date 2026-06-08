resource "aws_ssm_activation" "activation" {
  description        = local.ssm_activation_description
  iam_role           = aws_iam_role.instance_role.id
  name               = local.ssm_activation_name
  registration_limit = var.ssm_activation_registration_limit
  tags = merge(
    {
      Name = local.ssm_activation_name
    },
    var.tags
  )
}
