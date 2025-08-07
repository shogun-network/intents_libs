use graphql_client::GraphQLQuery;

pub mod pricing;
// https://docs.codex.io/api-reference/introduction
const CODEX_API_URL: &str = "https://graph.codex.io/graphql";

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/graphql/codex.graphql",
    query_path = "tests/unions/union_query.graphql",
    skip_serializing_none
)]
struct UnionQuery;
