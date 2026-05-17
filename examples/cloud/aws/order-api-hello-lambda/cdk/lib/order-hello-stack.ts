import * as cdk from 'aws-cdk-lib';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as integrations from 'aws-cdk-lib/aws-apigatewayv2-integrations';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as path from 'path';
import type { Construct } from 'constructs';

export class OrderHelloStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // Resolve `../dist` relative to `cdk/` (run all CDK commands from `cdk/` — see README).
    const assetDir = path.resolve(process.cwd(), '..', 'dist');

    const orderHello = new lambda.Function(this, 'OrderHelloFunction', {
      functionName: 'kernl-order-hello',
      description: 'kernl demo — REST-ish routes backed by kn/order_api.knl',
      runtime: lambda.Runtime.PROVIDED_AL2023,
      architecture: lambda.Architecture.X86_64,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(assetDir),
      timeout: cdk.Duration.seconds(15),
      memorySize: 512,
    });

    const httpApi = new apigatewayv2.HttpApi(this, 'OrderHttpApi', {
      apiName: 'kernl-order-hello-http',
    });

    const integration = new integrations.HttpLambdaIntegration(
      'KernlApiIntegration',
      orderHello,
    );

    httpApi.addRoutes({
      path: '/health',
      methods: [apigatewayv2.HttpMethod.GET],
      integration,
    });
    httpApi.addRoutes({
      path: '/orders',
      methods: [
        apigatewayv2.HttpMethod.GET,
        apigatewayv2.HttpMethod.POST,
      ],
      integration,
    });
    httpApi.addRoutes({
      path: '/orders/{id}',
      methods: [
        apigatewayv2.HttpMethod.GET,
        apigatewayv2.HttpMethod.PATCH,
        apigatewayv2.HttpMethod.DELETE,
      ],
      integration,
    });
    httpApi.addRoutes({
      path: '/customers',
      methods: [apigatewayv2.HttpMethod.GET],
      integration,
    });

    const base = httpApi.url ?? '';

    new cdk.CfnOutput(this, 'HttpApiBaseUrl', {
      description: 'Base URL of the HTTP API',
      value: base,
    });

    new cdk.CfnOutput(this, 'HealthUrl', {
      description: 'GET health',
      value: cdk.Fn.join('', [base, 'health']),
    });

    new cdk.CfnOutput(this, 'OrdersListUrl', {
      description: 'GET orders (fake list)',
      value: cdk.Fn.join('', [base, 'orders']),
    });

    new cdk.CfnOutput(this, 'OrderDetailExampleUrl', {
      description: 'GET single order (example id)',
      value: cdk.Fn.join('', [base, 'orders/ord-1001']),
    });

    new cdk.CfnOutput(this, 'CustomersListUrl', {
      description: 'GET customers (fake list)',
      value: cdk.Fn.join('', [base, 'customers']),
    });
  }
}
