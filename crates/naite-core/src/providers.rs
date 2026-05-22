use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::repo::Repository;
use crate::{command, Error};

const GITHUB_COMMIT_AVATAR_CHUNK_SIZE: usize = 50;
const GITHUB_PR_LIST_FIELDS: &str = concat!(
    "number,title,headRefName,baseRefName,author,isDraft,reviewDecision,",
    "mergeStateStatus,updatedAt,url,statusCheckRollup,labels,reviewRequests,",
    "closingIssuesReferences"
);
const GITHUB_PR_LIST_JQ: &str = r##".[] | [
    .number,
    .title,
    .headRefName,
    .baseRefName,
    .author.login,
    (.author.avatarUrl // ""),
    (.labels | map(.name) | join(",")),
    .isDraft,
    (.reviewDecision // ""),
    (.mergeStateStatus // ""),
    .updatedAt,
    .url,
    ([.statusCheckRollup[]? | (.conclusion // .status // "")] | map(select(. != "")) | unique | join(",")),
    ([.reviewRequests[]? | (.login // .name // .slug // .requestedReviewer.login // "")] | map(select(. != "")) | join(",")),
    ([.closingIssuesReferences[]? | ("#" + (.number | tostring) + " " + .url)] | join(","))
] | @tsv"##;
const GITHUB_ISSUE_LIST_FIELDS: &str = "number,title,state,author,labels,updatedAt,url";
const GITHUB_ISSUE_LIST_JQ: &str = r##".[] | [
    .number,
    .title,
    .state,
    .author.login,
    (.labels | map(.name) | join(",")),
    .updatedAt,
    .url
] | @tsv"##;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostingProvider {
    GitHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestFilter {
    All,
    Mine,
    NeedsReview,
    Draft,
    FailingChecks,
    CurrentBranch,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestReviewStatus {
    Approved,
    ChangesRequested,
    ReviewRequired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestCiStatus {
    Passing,
    Failing,
    Pending,
    NoChecks,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubIssueFilter {
    Open,
    Assigned,
    Mentioned,
    Closed,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueLink {
    pub number: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSummary {
    pub provider: HostingProvider,
    pub number: u32,
    pub title: String,
    pub head_branch: String,
    pub base_branch: String,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub reviewers: Vec<String>,
    pub labels: Vec<String>,
    pub draft: bool,
    pub review_status: PullRequestReviewStatus,
    pub ci_status: PullRequestCiStatus,
    pub merge_state: String,
    pub updated_at: String,
    pub url: String,
    pub issue_links: Vec<IssueLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssueSummary {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitAuthorAvatar {
    pub commit_id: String,
    pub author_avatar_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatePullRequestOptions {
    pub base_branch: Option<String>,
    pub draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPullRequestsOptions {
    pub filter: PullRequestFilter,
    pub search_query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListGitHubIssuesOptions {
    pub filter: GitHubIssueFilter,
    pub search_query: Option<String>,
}

impl Default for ListGitHubIssuesOptions {
    fn default() -> Self {
        Self {
            filter: GitHubIssueFilter::Open,
            search_query: None,
        }
    }
}

impl Default for ListPullRequestsOptions {
    fn default() -> Self {
        Self {
            filter: PullRequestFilter::All,
            search_query: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckoutPullRequestOptions {
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,
}

impl Repository {
    pub fn list_pull_requests(
        &self,
        provider: HostingProvider,
        options: ListPullRequestsOptions,
    ) -> Result<Vec<PullRequestSummary>, Error> {
        match provider {
            HostingProvider::GitHub => self.list_github_pull_requests(options),
        }
    }

    pub fn create_pull_request(
        &self,
        provider: HostingProvider,
        options: CreatePullRequestOptions,
    ) -> Result<String, Error> {
        match provider {
            HostingProvider::GitHub => self.create_github_pull_request(options),
        }
    }

    pub fn checkout_pull_request(
        &self,
        provider: HostingProvider,
        number: u32,
        options: CheckoutPullRequestOptions,
    ) -> Result<(), Error> {
        match provider {
            HostingProvider::GitHub => self.checkout_github_pull_request(number, options),
        }
    }

    pub fn open_pull_request_in_browser(
        &self,
        provider: HostingProvider,
        number: u32,
    ) -> Result<(), Error> {
        match provider {
            HostingProvider::GitHub => {
                let args = vec![
                    "pr".to_string(),
                    "view".to_string(),
                    number.to_string(),
                    "--web".to_string(),
                ];
                let _ = self.run_gh(args)?;
                Ok(())
            }
        }
    }

    pub fn list_github_issues(
        &self,
        options: ListGitHubIssuesOptions,
    ) -> Result<Vec<GitHubIssueSummary>, Error> {
        let args = build_github_issue_list_args(&options)?;
        let output = self.run_gh(args)?;
        parse_github_issue_list(&output)
    }

    pub fn open_github_issue_in_browser(&self, number: u32) -> Result<(), Error> {
        let args = vec![
            "issue".to_string(),
            "view".to_string(),
            number.to_string(),
            "--web".to_string(),
        ];
        let _ = self.run_gh(args)?;
        Ok(())
    }

    pub fn resolve_github_commit_author_avatars(
        &self,
        commit_ids: &[String],
    ) -> Result<Vec<CommitAuthorAvatar>, Error> {
        if commit_ids.is_empty() {
            return Ok(Vec::new());
        }

        let (owner, name) = self.github_repository_slug()?;
        let mut avatars = Vec::new();
        for chunk in commit_ids.chunks(GITHUB_COMMIT_AVATAR_CHUNK_SIZE) {
            let args = build_github_commit_avatar_graphql_args(&owner, &name, chunk);
            let output = self.run_gh(args)?;
            avatars.extend(parse_github_commit_avatar_rows(&output));
        }

        Ok(avatars)
    }

    fn list_github_pull_requests(
        &self,
        options: ListPullRequestsOptions,
    ) -> Result<Vec<PullRequestSummary>, Error> {
        let args = build_github_pr_list_args(&options, self.head_branch())?;
        let output = self.run_gh(args)?;
        parse_github_pr_list(&output)
    }

    fn create_github_pull_request(
        &self,
        options: CreatePullRequestOptions,
    ) -> Result<String, Error> {
        let branch = self.head_branch().ok_or(Error::NoCurrentBranch)?;
        let args = build_github_pr_create_args(&branch, &options);
        let output = self.run_gh(args)?;
        Ok(extract_created_pr_url(&output))
    }

    fn run_gh(&self, args: Vec<String>) -> Result<String, Error> {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_provider_cli("gh", cwd, args)
    }

    fn github_repository_slug(&self) -> Result<(String, String), Error> {
        let output = self.run_gh(vec![
            "repo".to_string(),
            "view".to_string(),
            "--json".to_string(),
            "owner,name".to_string(),
            "--jq".to_string(),
            ".owner.login + \"\\t\" + .name".to_string(),
        ])?;
        parse_github_repository_slug(&output)
    }

    fn checkout_github_pull_request(
        &self,
        number: u32,
        options: CheckoutPullRequestOptions,
    ) -> Result<(), Error> {
        match options.worktree_path {
            Some(path) => {
                let path = validate_provider_worktree_path(&path)?;
                let cwd = self.workdir().unwrap_or(self.path());
                let _ = command::run_git(
                    cwd,
                    [
                        OsStr::new("worktree"),
                        OsStr::new("add"),
                        OsStr::new("--detach"),
                        path.as_os_str(),
                        OsStr::new("HEAD"),
                    ],
                )?;

                let args = build_github_pr_checkout_args(number, options.branch_name);
                match command::run_provider_cli("gh", path, args) {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        let _ = command::run_git(
                            cwd,
                            [
                                OsStr::new("worktree"),
                                OsStr::new("remove"),
                                OsStr::new("--force"),
                                path.as_os_str(),
                            ],
                        );
                        Err(err)
                    }
                }
            }
            None => {
                let args = build_github_pr_checkout_args(number, options.branch_name);
                let _ = self.run_gh(args)?;
                Ok(())
            }
        }
    }
}

fn build_github_issue_list_args(options: &ListGitHubIssuesOptions) -> Result<Vec<String>, Error> {
    let mut args = vec![
        "issue".to_string(),
        "list".to_string(),
        "--limit".to_string(),
        "50".to_string(),
        "--json".to_string(),
        GITHUB_ISSUE_LIST_FIELDS.to_string(),
        "--jq".to_string(),
        GITHUB_ISSUE_LIST_JQ.to_string(),
    ];

    match options.filter {
        GitHubIssueFilter::Open => {
            args.push("--state".to_string());
            args.push("open".to_string());
        }
        GitHubIssueFilter::Assigned => {
            args.push("--state".to_string());
            args.push("open".to_string());
            args.push("--assignee".to_string());
            args.push("@me".to_string());
        }
        GitHubIssueFilter::Mentioned => {
            args.push("--state".to_string());
            args.push("open".to_string());
            args.push("--mention".to_string());
            args.push("@me".to_string());
        }
        GitHubIssueFilter::Closed => {
            args.push("--state".to_string());
            args.push("closed".to_string());
        }
        GitHubIssueFilter::Search => {
            let query = options
                .search_query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
                .ok_or_else(|| Error::InvalidRefName("empty issue search".into()))?;
            args.push("--search".to_string());
            args.push(query.to_string());
        }
    }

    Ok(args)
}

fn build_github_commit_avatar_graphql_args(
    owner: &str,
    name: &str,
    commit_ids: &[String],
) -> Vec<String> {
    let fields = commit_ids
        .iter()
        .enumerate()
        .map(|(index, commit_id)| {
            format!(
                "c{index}: object(oid:\"{commit_id}\") {{ ... on Commit {{ oid author {{ user {{ avatarUrl }} }} }} }}"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let query = format!(
        "query($owner:String!, $name:String!) {{ repository(owner:$owner, name:$name) {{ {fields} }} }}"
    );

    vec![
        "api".to_string(),
        "graphql".to_string(),
        "-F".to_string(),
        format!("owner={owner}"),
        "-F".to_string(),
        format!("name={name}"),
        "-f".to_string(),
        format!("query={query}"),
        "--jq".to_string(),
        ".data.repository | to_entries[] | [.value.oid, (.value.author.user.avatarUrl // \"\")] | @tsv".to_string(),
    ]
}

fn build_github_pr_list_args(
    options: &ListPullRequestsOptions,
    head_branch: Option<String>,
) -> Result<Vec<String>, Error> {
    let mut args = vec![
        "pr".to_string(),
        "list".to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--limit".to_string(),
        "50".to_string(),
        "--json".to_string(),
        GITHUB_PR_LIST_FIELDS.to_string(),
        "--jq".to_string(),
        GITHUB_PR_LIST_JQ.to_string(),
    ];

    match options.filter {
        PullRequestFilter::All => {}
        PullRequestFilter::Mine => {
            args.push("--author".to_string());
            args.push("@me".to_string());
        }
        PullRequestFilter::NeedsReview => {
            args.push("--search".to_string());
            args.push("review:required".to_string());
        }
        PullRequestFilter::Draft => {
            args.push("--draft".to_string());
        }
        PullRequestFilter::FailingChecks => {
            args.push("--search".to_string());
            args.push("status:failure".to_string());
        }
        PullRequestFilter::CurrentBranch => {
            let branch = head_branch.ok_or(Error::NoCurrentBranch)?;
            args.push("--head".to_string());
            args.push(branch);
        }
        PullRequestFilter::Search => {
            let query = options
                .search_query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
                .ok_or_else(|| Error::InvalidRefName("empty pull request search".into()))?;
            args.push("--search".to_string());
            args.push(query.to_string());
        }
    }

    Ok(args)
}

fn build_github_pr_create_args(branch: &str, options: &CreatePullRequestOptions) -> Vec<String> {
    let mut args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--fill".to_string(),
        "--head".to_string(),
        branch.to_string(),
    ];
    if let Some(base) = options
        .base_branch
        .as_deref()
        .map(str::trim)
        .filter(|base| !base.is_empty())
    {
        args.push("--base".to_string());
        args.push(base.to_string());
    }
    if options.draft {
        args.push("--draft".to_string());
    }
    args
}

fn build_github_pr_checkout_args(number: u32, branch_name: Option<String>) -> Vec<String> {
    let mut args = vec!["pr".to_string(), "checkout".to_string(), number.to_string()];
    if let Some(branch) = branch_name
        .as_deref()
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
    {
        args.push("--branch".to_string());
        args.push(branch.to_string());
    }
    args
}

fn parse_github_repository_slug(output: &str) -> Result<(String, String), Error> {
    let mut fields = output.trim().split('\t');
    let owner = fields.next().unwrap_or_default();
    let name = fields.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || fields.next().is_some() {
        return Err(Error::ProviderCommand {
            command: "gh repo view".into(),
            stderr: format!("unexpected repository metadata: {}", output.trim()),
        });
    }

    Ok((owner.to_string(), name.to_string()))
}

fn parse_github_pr_list(output: &str) -> Result<Vec<PullRequestSummary>, Error> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_github_pr_line)
        .collect()
}

fn parse_github_commit_avatar_rows(output: &str) -> Vec<CommitAuthorAvatar> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(2, '\t');
            let commit_id = fields.next()?.trim();
            if commit_id.is_empty() {
                return None;
            }
            let author_avatar_url = fields
                .next()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string);

            Some(CommitAuthorAvatar {
                commit_id: commit_id.to_string(),
                author_avatar_url,
            })
        })
        .collect()
}

fn parse_github_pr_line(line: &str) -> Result<PullRequestSummary, Error> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != 15 {
        return Err(Error::ProviderCommand {
            command: "gh pr list".into(),
            stderr: format!("unexpected pull request output: {line}"),
        });
    }

    let number = fields[0]
        .parse::<u32>()
        .map_err(|_| Error::ProviderCommand {
            command: "gh pr list".into(),
            stderr: format!("invalid pull request number: {}", fields[0]),
        })?;

    Ok(PullRequestSummary {
        provider: HostingProvider::GitHub,
        number,
        title: unescape_tsv_field(fields[1]),
        head_branch: unescape_tsv_field(fields[2]),
        base_branch: unescape_tsv_field(fields[3]),
        author: unescape_tsv_field(fields[4]),
        author_avatar_url: optional_tsv_field(fields[5]),
        labels: split_csv_field(fields[6]),
        reviewers: split_csv_field(fields[13]),
        draft: fields[7] == "true",
        review_status: review_status_from_field(fields[8]),
        ci_status: ci_status_from_rollup(fields[12]),
        merge_state: unescape_tsv_field(fields[9]),
        updated_at: unescape_tsv_field(fields[10]),
        url: unescape_tsv_field(fields[11]),
        issue_links: parse_issue_links(fields[14]),
    })
}

fn parse_github_issue_list(output: &str) -> Result<Vec<GitHubIssueSummary>, Error> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_github_issue_line)
        .collect()
}

fn parse_github_issue_line(line: &str) -> Result<GitHubIssueSummary, Error> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != 7 {
        return Err(Error::ProviderCommand {
            command: "gh issue list".into(),
            stderr: format!("unexpected issue output: {line}"),
        });
    }

    let number = fields[0]
        .parse::<u32>()
        .map_err(|_| Error::ProviderCommand {
            command: "gh issue list".into(),
            stderr: format!("invalid issue number: {}", fields[0]),
        })?;

    Ok(GitHubIssueSummary {
        number,
        title: unescape_tsv_field(fields[1]),
        state: unescape_tsv_field(fields[2]),
        author: unescape_tsv_field(fields[3]),
        labels: split_csv_field(fields[4]),
        updated_at: unescape_tsv_field(fields[5]),
        url: unescape_tsv_field(fields[6]),
    })
}

fn split_csv_field(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(|part| {
            let part = unescape_tsv_field(part).trim().to_string();
            (!part.is_empty()).then_some(part)
        })
        .collect()
}

fn optional_tsv_field(value: &str) -> Option<String> {
    let value = unescape_tsv_field(value);
    (!value.trim().is_empty()).then_some(value)
}

fn parse_issue_links(value: &str) -> Vec<IssueLink> {
    value
        .split(',')
        .filter_map(|part| {
            let (number, url) = part.trim().split_once(' ')?;
            let number = number.strip_prefix('#')?.parse::<u32>().ok()?;
            Some(IssueLink {
                number,
                url: url.to_string(),
            })
        })
        .collect()
}

fn review_status_from_field(value: &str) -> PullRequestReviewStatus {
    match value {
        "APPROVED" => PullRequestReviewStatus::Approved,
        "CHANGES_REQUESTED" => PullRequestReviewStatus::ChangesRequested,
        "REVIEW_REQUIRED" => PullRequestReviewStatus::ReviewRequired,
        _ => PullRequestReviewStatus::Unknown,
    }
}

fn ci_status_from_rollup(value: &str) -> PullRequestCiStatus {
    let statuses = value
        .split(',')
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .collect::<Vec<_>>();

    if statuses.is_empty() {
        return PullRequestCiStatus::NoChecks;
    }
    if statuses.iter().any(|status| {
        matches!(
            *status,
            "ACTION_REQUIRED"
                | "CANCELLED"
                | "FAILURE"
                | "FAILED"
                | "ERROR"
                | "STARTUP_FAILURE"
                | "TIMED_OUT"
        )
    }) {
        return PullRequestCiStatus::Failing;
    }
    if statuses.iter().any(|status| {
        matches!(
            *status,
            "EXPECTED" | "IN_PROGRESS" | "PENDING" | "QUEUED" | "REQUESTED" | "WAITING"
        )
    }) {
        return PullRequestCiStatus::Pending;
    }
    if statuses
        .iter()
        .all(|status| matches!(*status, "SUCCESS" | "NEUTRAL" | "SKIPPED"))
    {
        return PullRequestCiStatus::Passing;
    }

    PullRequestCiStatus::Unknown
}

fn extract_created_pr_url(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .unwrap_or_else(|| output.trim())
        .to_string()
}

fn unescape_tsv_field(value: &str) -> String {
    value
        .replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\\\", "\\")
}

fn validate_provider_worktree_path(path: &Path) -> Result<&Path, Error> {
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidPath(path.display().to_string()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_pr_list_reads_metadata_statuses_and_issue_links() {
        let output = concat!(
            "42\tAdd PR support\tfeature/prs\tmain\tjune\thttps://avatars.githubusercontent.com/u/1?v=4\tui,github\tfalse\tAPPROVED\t",
            "CLEAN\t2026-05-18T01:02:03Z\thttps://github.com/acme/repo/pull/42\t",
            "SUCCESS\tada,grace\t#7 https://github.com/acme/repo/issues/7\n",
            "43\tFix checks\tfix/checks\tmain\talex\t\t\ttrue\tCHANGES_REQUESTED\t",
            "BLOCKED\t2026-05-18T02:03:04Z\thttps://github.com/acme/repo/pull/43\t",
            "FAILURE,PENDING\t\t\n",
        );

        let prs = parse_github_pr_list(output).unwrap();

        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].number, 42);
        assert_eq!(
            prs[0].author_avatar_url.as_deref(),
            Some("https://avatars.githubusercontent.com/u/1?v=4")
        );
        assert_eq!(prs[0].labels, vec!["ui", "github"]);
        assert_eq!(prs[0].reviewers, vec!["ada", "grace"]);
        assert_eq!(prs[0].review_status, PullRequestReviewStatus::Approved);
        assert_eq!(prs[0].ci_status, PullRequestCiStatus::Passing);
        assert_eq!(prs[0].issue_links[0].number, 7);
        assert!(prs[1].draft);
        assert_eq!(
            prs[1].review_status,
            PullRequestReviewStatus::ChangesRequested
        );
        assert_eq!(prs[1].ci_status, PullRequestCiStatus::Failing);
    }

    #[test]
    fn parse_github_repository_slug_reads_owner_and_name() {
        assert_eq!(
            parse_github_repository_slug("easdkr\tnaite\n").unwrap(),
            ("easdkr".into(), "naite".into())
        );
        assert!(parse_github_repository_slug("invalid\n").is_err());
    }

    #[test]
    fn parse_github_commit_avatar_rows_reads_optional_urls() {
        let rows = parse_github_commit_avatar_rows(
            "abc123\thttps://avatars.githubusercontent.com/u/1?v=4\n\
             def456\t\n",
        );

        assert_eq!(
            rows,
            vec![
                CommitAuthorAvatar {
                    commit_id: "abc123".into(),
                    author_avatar_url: Some("https://avatars.githubusercontent.com/u/1?v=4".into()),
                },
                CommitAuthorAvatar {
                    commit_id: "def456".into(),
                    author_avatar_url: None,
                },
            ]
        );
    }

    #[test]
    fn parse_github_issue_list_reads_basic_metadata() {
        let output = concat!(
            "7\tFix focus ring\tOPEN\tjune\tui,polish\t2026-05-18T01:02:03Z\thttps://github.com/acme/repo/issues/7\n",
            "8\tPersist density\tCLOSED\talex\tsettings\t2026-05-17T01:02:03Z\thttps://github.com/acme/repo/issues/8\n",
        );

        let issues = parse_github_issue_list(output).unwrap();

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].number, 7);
        assert_eq!(issues[0].title, "Fix focus ring");
        assert_eq!(issues[0].labels, vec!["ui", "polish"]);
        assert_eq!(issues[1].state, "CLOSED");
    }

    #[test]
    fn issue_search_filter_requires_query() {
        let err = build_github_issue_list_args(&ListGitHubIssuesOptions {
            filter: GitHubIssueFilter::Search,
            search_query: Some(" ".into()),
        })
        .unwrap_err();

        assert!(matches!(err, Error::InvalidRefName(_)));
    }

    #[test]
    fn current_branch_filter_requires_a_branch() {
        let err = build_github_pr_list_args(
            &ListPullRequestsOptions {
                filter: PullRequestFilter::CurrentBranch,
                search_query: None,
            },
            None,
        )
        .unwrap_err();

        assert!(matches!(err, Error::NoCurrentBranch));
    }

    #[test]
    fn current_branch_filter_adds_head_flag() {
        let args = build_github_pr_list_args(
            &ListPullRequestsOptions {
                filter: PullRequestFilter::CurrentBranch,
                search_query: None,
            },
            Some("feature/prs".into()),
        )
        .unwrap();

        assert!(args
            .windows(2)
            .any(|pair| pair == ["--head", "feature/prs"]));
    }

    #[test]
    fn custom_search_filter_adds_search_query() {
        let args = build_github_pr_list_args(
            &ListPullRequestsOptions {
                filter: PullRequestFilter::Search,
                search_query: Some("status:success author:@me".into()),
            },
            Some("feature/prs".into()),
        )
        .unwrap();

        assert!(args
            .windows(2)
            .any(|pair| pair == ["--search", "status:success author:@me"]));
    }

    #[test]
    fn custom_search_filter_requires_query() {
        let err = build_github_pr_list_args(
            &ListPullRequestsOptions {
                filter: PullRequestFilter::Search,
                search_query: Some(" ".into()),
            },
            Some("feature/prs".into()),
        )
        .unwrap_err();

        assert!(matches!(err, Error::InvalidRefName(_)));
    }

    #[test]
    fn create_pr_args_use_current_branch_head_and_optional_base() {
        let args = build_github_pr_create_args(
            "feature/prs",
            &CreatePullRequestOptions {
                base_branch: Some("main".into()),
                draft: true,
            },
        );

        assert_eq!(
            args,
            vec![
                "pr",
                "create",
                "--fill",
                "--head",
                "feature/prs",
                "--base",
                "main",
                "--draft",
            ]
        );
    }

    #[test]
    fn checkout_pr_args_use_optional_branch_name() {
        assert_eq!(
            build_github_pr_checkout_args(42, Some("pr-42".into())),
            vec!["pr", "checkout", "42", "--branch", "pr-42"]
        );
        assert_eq!(
            build_github_pr_checkout_args(42, Some(" ".into())),
            vec!["pr", "checkout", "42"]
        );
    }

    #[test]
    fn ci_rollup_prefers_failure_over_pending() {
        assert_eq!(
            ci_status_from_rollup("SUCCESS,PENDING,FAILURE"),
            PullRequestCiStatus::Failing
        );
        assert_eq!(
            ci_status_from_rollup("SUCCESS,NEUTRAL"),
            PullRequestCiStatus::Passing
        );
        assert_eq!(ci_status_from_rollup(""), PullRequestCiStatus::NoChecks);
    }

    #[test]
    fn created_pr_url_uses_first_url_line() {
        let url = extract_created_pr_url("Opening browser\nhttps://github.com/acme/repo/pull/9\n");

        assert_eq!(url, "https://github.com/acme/repo/pull/9");
    }
}
