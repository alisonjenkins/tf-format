resource "aws_route53_record" "cjwdesign_validation" {
  for_each = {
    for dvo in aws_acm_certificate.cjwdesign.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }
}
