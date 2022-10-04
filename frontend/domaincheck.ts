import domains from "./domains.json" assert { type: "json" };

export function domainCheck(
  currentDomain: string,
  requestedUrl: string,
): boolean {
  let foundDomain: string | undefined;
  for (const domain of domains) {
    if (currentDomain.endsWith(domain)) {
      foundDomain = domain;
      break;
    }
  }
  if (foundDomain === undefined) return false;

  const currentDomainSplit = currentDomain.replace(foundDomain, "").split(".");
  const requestedUrlSplit = requestedUrl.replace(foundDomain, "").split(".");
  return currentDomainSplit[currentDomainSplit.length - 1] ===
    requestedUrlSplit[requestedUrlSplit.length - 1];
}
