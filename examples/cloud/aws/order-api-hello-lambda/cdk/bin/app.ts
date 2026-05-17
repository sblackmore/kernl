#!/usr/bin/env node
import * as cdk from 'aws-cdk-lib';
import { OrderHelloStack } from '../lib/order-hello-stack';

const app = new cdk.App();

new OrderHelloStack(app, 'KernlOrderHelloStack', {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: process.env.CDK_DEFAULT_REGION ?? 'us-east-1',
  },
  description: 'HTTP API + Lambda — kernl order_hello.knl via kernlc (zip / provided.al2023)',
});
