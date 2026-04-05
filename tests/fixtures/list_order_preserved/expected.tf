resource "aws_instance" "example" {
  for_each = toset(["c", "a", "b"])

  ami                = "ami-12345678"
  security_group_ids = ["sg-zzz", "sg-aaa", "sg-mmm"]

  availability_zones = [
    var.zone_primary,
    var.zone_secondary,
    var.zone_tertiary,
  ]

  subnet_ids = [
    "subnet-ccc",
    "subnet-aaa",
    "subnet-bbb",
  ]
}
