return {
  name = "http_example",
  description = "Example Lua tool that fetches a URL and returns first 200 chars.",
  entry = "run",
  args = {
    { name = "url", description = "URL to fetch", type = "string", required = true }
  },
  run = function(args)
    local body = http.get(args.url)
    if not body then return "" end
    return string.sub(body, 1, 200)
  end
}
