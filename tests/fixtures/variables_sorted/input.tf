variable "zone" {
  type    = string
  default = "us-east-1a"
}

variable "ami" {
  type        = string
  description = "The AMI to use"
}

variable "instance_type" {
  type    = string
  default = "t2.micro"
}
