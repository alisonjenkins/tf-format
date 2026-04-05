variable "ami" {
  description = "The AMI to use"
  type        = string
}

variable "instance_type" {
  default = "t2.micro"
  type    = string
}

variable "zone" {
  default = "us-east-1a"
  type    = string
}
