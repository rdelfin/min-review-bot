use clap::Parser;

mod pr;

#[derive(Parser, Debug)] // requires `derive` feature
#[command(term_width = 0)] // Just to make testing across clap features easier
struct Args {
    /// Implicitly using `std::str::FromStr`
    #[arg(long, short)]
    pr_num: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("PR: {:?}", pr::get_pr(args.pr_num).await?);
    Ok(())
}
