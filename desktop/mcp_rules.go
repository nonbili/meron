package main

import "github.com/modelcontextprotocol/go-sdk/mcp"

type mcpRuleCondition struct {
	Field  string `json:"field" jsonschema:"One of from, to, subject, body, subject_or_body, tag, attachment, unread, starred, or words (Meron header search syntax)"`
	Value  string `json:"value,omitempty" jsonschema:"Match text or a tag. From, to and subject accept comma-separated alternatives. Omit for attachment, unread and starred."`
	Negate bool   `json:"negate,omitempty" jsonschema:"Invert the condition. Defaults to false."`
}

type mcpRule struct {
	Conditions    *[]mcpRuleCondition `json:"conditions,omitempty" jsonschema:"Conditions to match. An empty list matches all mail in scope."`
	MatchAny      *bool               `json:"match_any,omitempty" jsonschema:"Match when any condition holds. Defaults to requiring all conditions."`
	Trigger       *string             `json:"trigger,omitempty" jsonschema:"arrival (default), read (after the message becomes read), or age (can apply to existing Inbox mail and must be enabled in Meron)"`
	DelaySeconds  *int64              `json:"delay_seconds,omitempty" jsonschema:"Seconds after the trigger. Defaults to zero."`
	Tags          *[]string           `json:"tags,omitempty" jsonschema:"Tags to add to matching mail"`
	MarkRead      *bool               `json:"mark_read,omitempty" jsonschema:"Mark matching mail read"`
	Star          *bool               `json:"star,omitempty" jsonschema:"Star matching mail"`
	MoveTo        *string             `json:"move_to,omitempty" jsonschema:"archive, trash, or folder. An empty string leaves mail in place."`
	Folder        *string             `json:"folder,omitempty" jsonschema:"Destination folder ID when move_to is folder"`
	FolderAccount *string             `json:"folder_account,omitempty" jsonschema:"Account ID of the move destination. Defaults to the message's account."`
	Accounts      *[]string           `json:"accounts,omitempty" jsonschema:"Source mail account IDs. An empty list applies to all current and future mail accounts."`
	CopyFolder    *string             `json:"copy_folder,omitempty" jsonschema:"Destination folder ID for a copy. Requires copy_account."`
	CopyAccount   *string             `json:"copy_account,omitempty" jsonschema:"Destination account ID for a copy. Requires copy_folder."`
	ForwardTo     *string             `json:"forward_to,omitempty" jsonschema:"Comma-separated addresses to automatically forward matching mail to"`
	Disabled      *bool               `json:"disabled,omitempty" jsonschema:"Whether the rule is turned off. New MCP rules default to disabled."`
	Sent          *bool               `json:"sent,omitempty" jsonschema:"Apply to sent mail instead of received mail. Sent rules run immediately, cannot check the body, and can only tag, star or copy."`
}

type mcpCreateRuleArgs struct {
	Rule mcpRule `json:"rule" jsonschema:"Rule definition with at least one action"`
}

type mcpRuleIDArgs struct {
	RuleID string `json:"rule_id" jsonschema:"Exact id returned by list_rules or create_rule"`
}

type mcpUpdateRuleArgs struct {
	mcpRuleIDArgs
	Rule mcpRule `json:"rule" jsonschema:"Fields to change. Omitted fields keep their saved values."`
}

func (s *mcpService) registerRuleTools(server *mcp.Server, c mcpClient) {
	if !c.ManageRules {
		return
	}
	mcpRegister(s, server, c, "list_rules", "List this device's saved mail rules in evaluation order. Returns each rule's id and full definition.", mcpReadRules, func(struct{}) string { return "" }, func(_ struct{}, _ mcpClient) (any, error) {
		return s.coreCall("rules.list", struct{}{})
	})
	mcpRegister(s, server, c, "list_rule_targets", "List mail accounts and their cached folder IDs for rule sources and destinations. The optional local entry lists on-device folders that can be used as destinations.", mcpReadRules, func(struct{}) string { return "" }, func(_ struct{}, _ mcpClient) (any, error) {
		return s.coreCall("rules.targets", struct{}{})
	})
	mcpRegister(s, server, c, "create_rule", "Append a mail rule and return it with its new id. Rules are disabled by default. Set disabled to false to enable a new arrival or read rule. Age rules can act on existing Inbox mail and must be enabled in Meron. Omitted conditions match all mail and omitted accounts cover all mail accounts. Retrying creates another rule.", mcpManageRules, func(mcpCreateRuleArgs) string { return "" }, func(args mcpCreateRuleArgs, _ mcpClient) (any, error) {
		if args.Rule.Disabled == nil {
			disabled := true
			args.Rule.Disabled = &disabled
		}
		return s.coreCall("rules.create", args)
	})
	mcpRegister(s, server, c, "update_rule", "Edit one saved mail rule in place. Pass its rule_id and the fields to change in rule. Omitted fields keep their saved values, including accounts and disabled. Edits apply to future arrival and read events. Include disabled: true when changing an active age rule. Enable age rules in Meron after review.", mcpManageRules, func(mcpUpdateRuleArgs) string { return "" }, func(args mcpUpdateRuleArgs, _ mcpClient) (any, error) {
		return s.coreCall("rules.update", args)
	})
	mcpRegister(s, server, c, "delete_rule", "Remove one saved mail rule by its exact rule_id. Work already queued by that rule may still finish.", mcpManageRules, func(mcpRuleIDArgs) string { return "" }, func(args mcpRuleIDArgs, _ mcpClient) (any, error) {
		return s.coreCall("rules.delete", args)
	})
}
